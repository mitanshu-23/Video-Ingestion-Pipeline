use axum::extract::Multipart;
use tokio::io::AsyncWriteExt;

use crate::utils::storage::create_file;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkedUploadRequest {
    pub transfer_id: Option<String>,
    pub data: String,
    pub filename: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkedUploadResponse {
    pub upload_id: String,
    pub chunk_index: u32,
    pub message: String,
}

// Body of the init request: the client knows the file up front, so it declares
// how many chunks it will send. This is the single source of truth for the
// completion target — chunk requests no longer carry it.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkedUploadInitRequest {
    pub total_chunks: u32,
    // The file's MIME type, declared once up front. The whole file has a single
    // type, so it's pinned here at init and every chunk is checked against it
    // instead of trusting each chunk request's own Content-Type header.
    pub content_type: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkedUploadInitResponse {
    pub upload_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkQuery {
    pub upload_id: String,
    pub chunk_index: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDetails {
    pub transfer_id: String,
    pub status: ProcessingStatus,
    pub file_name: String,
    // Total number of chunks the client committed to at init time. Fixed for
    // the transfer's life, so it's the reliable completion target.
    pub total_chunks: u32,
    // The set of chunk indices actually persisted so far. A set (not a counter)
    // so a retried/duplicate chunk can't inflate progress and make an upload
    // look complete while a distinct chunk is still missing.
    pub received_chunks: std::collections::HashSet<u32>,
    pub extension: String,
    pub chunk_type: ChunkType,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableUploadInitRequest {
    pub total_chunks: u32,
    // The file's MIME type, declared once up front. The whole file has a single
    // type, so it's pinned here at init and every chunk is checked against it
    // instead of trusting each chunk request's own Content-Type header.
    pub content_type: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableUploadInitResponse {
    pub upload_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableChunkQuery {
    pub upload_id: String,
    pub chunk_index: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableUploadResponse {
    pub upload_id: String,
    pub chunk_index: u32,
    pub message: String,
}

// The client hits this before (re)sending chunks to learn what the server
// already has, so an interrupted transfer can pick up where it left off
// instead of re-uploading from chunk 0. Only `upload_id` is needed — the
// server owns every other fact about the transfer.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableStatusQuery {
    pub upload_id: String,
}

// The server's answer to "what do you have for this transfer?". The client
// diffs `received_chunks` (sorted, distinct) against `0..total_chunks` to find
// the indices it still needs to send.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumableStatusResponse {
    pub upload_id: String,
    pub total_chunks: u32,
    pub received_chunks: Vec<u32>,
    pub complete: bool,
}

// The exact shape we expect from a `multipart/form-data` body:
//   file: the video file to upload (binary part)
//   name: the name of the video (text part)
#[derive(Debug)]
pub struct MultipartReq {
    pub file: Vec<u8>,
    pub file_content_type: Option<String>,
    pub _name: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ChunkType {
    Raw,
    Resumable,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProcessingStatus {
    InProcess,
    None,
}

impl MultipartReq {
    // Drive the multipart stream field-by-field and assemble the typed struct.
    // Unknown fields are rejected so the body must match the contract exactly.
    pub async fn from_multipart(mut multipart: Multipart) -> Result<Self, String> {
        let mut file: Option<Vec<u8>> = None;
        let mut file_content_type: Option<String> = None;
        let mut name: Option<String> = None;

        while let Some(field) = multipart.next_field().await.map_err(|e| format!("malformed multipart body: {e}"))? {
            // `field.name()` borrows, so capture what we need before consuming it.
            match field.name() {
                Some("file") => {
                    // content_type() must be read before bytes() consumes the field.
                    file_content_type = field.content_type().map(|s| s.to_string());
                    name = field.file_name().map(|s| s.to_string());
                    let bytes = field.bytes().await.map_err(|e| format!("failed to read `file` field: {e}"))?;
                    log::info!("multipart upload | file={} size={}B", name.clone().unwrap_or_default(), bytes.len());
                    file = Some(bytes.to_vec());
                }
                Some(other) => {
                    log::info!("multipart field `{other}` is not expected; ignoring");
                }
                None => return Err("multipart field is missing a name".to_string()),
            }
        }

        Ok(MultipartReq { file: file.ok_or("missing required field `file`")?, file_content_type, _name: name.ok_or("missing required field `name`")? })
    }

    pub async fn stream_to_file(mut multipart: Multipart) -> Result<String, String> {
        // Holds the on-disk path of the written file; the caller canonicalizes
        // this, so it must be the full path, not the bare filename.
        let mut saved_path = "".to_string();

        while let Some(mut field) = multipart.next_field().await.map_err(|e| format!("malformed multipart body: {e}"))? {
            // `field.name()` borrows, so capture what we need before consuming it.
            match field.name() {
                Some("file") => {
                    // create file here

                    let _file_content_type = field.content_type().map(|s| s.to_string());

                    let file_name = field.file_name().map(|s| s.to_string()).ok_or("missing file name")?;
                    if file_name.is_empty() {
                        return Err("empty file name".to_string());
                    }
                    let full_path = format!("./uploads/multipart_stream/{}", file_name);
                    saved_path = full_path.clone();

                    let mut file = match create_file(&full_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            log::error!("Multipart - stream_to_file >> Failed to create file `{}`: {}, file:{}, line:{}", full_path, e, file!(), line!());
                            return Err("Error creating file".to_string());
                        }
                    };

                    // Stream the field's bytes to the file on disk in chunks.
                    while let Some(chunk) = field.chunk().await.map_err(|e| format!("failed to read `file` field chunk: {e}"))? {
                        if let Err(e) = file.write_all(&chunk).await {
                            log::error!("Multipart - stream_to_file >> failed to write chunk to file `{}`: {}, file:{}, line:{}", full_path, e, file!(), line!());
                            return Err("failed to write file chunk".to_string());
                        }

                        log::info!("Multipart - stream_to_file >> wrote chunk of {} bytes to `{}`", chunk.len(), full_path);
                    }

                    log::info!("Multipart - stream_to_file >> upload saved | file={} size={}B", full_path, file.metadata().await.map(|m| m.len()).unwrap_or(0));
                }
                Some(other) => {
                    log::info!("multipart field `{other}` is not expected; ignoring");
                }
                None => return Err("multipart field is missing a name".to_string()),
            }
        }

        if saved_path.is_empty() {
            return Err("missing required field `file`".to_string());
        }

        Ok(saved_path)
    }
}
