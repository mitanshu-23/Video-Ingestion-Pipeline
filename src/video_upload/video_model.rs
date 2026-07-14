use crate::{
    utils::{
        common::{generate_uuid, mime_to_extension, new_mime_id},
        state::{get_upload_state, set_upload_state, update_upload_state},
        storage::write_file,
    },
    video_upload::video_struct::{ChunkDetails, ChunkQuery, ChunkType, ChunkedUploadInitRequest, ChunkedUploadInitResponse, ChunkedUploadResponse, MultipartReq, ProcessingStatus, ResumableChunkQuery, ResumableStatusQuery, ResumableStatusResponse, ResumableUploadInitRequest, ResumableUploadInitResponse, ResumableUploadResponse},
};
use axum::{
    body::{BodyDataStream, Bytes},
    extract::Multipart,
    http::StatusCode,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

const ALLOWED_MIME_TYPES: &[&str] = &[
    // images
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    // documents
    "application/pdf",
    // video
    "video/mp4",
    "video/quicktime", // .mov
];

pub async fn video_upload_basic_http_body_handler(bytes: Bytes, content_type: String) -> Result<axum::Json<Value>, String> {
    // 1. Validate MIME
    let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("upload rejected: unsupported MIME type `{}`", mime_base);
        return Err(format!("unsupported file type: {mime_base}"));
    }

    if bytes.is_empty() {
        log::error!("upload rejected: empty body");
        return Err("empty body".to_string());
    }

    // 2. Generate filename + write to disk
    let extension = mime_to_extension(&mime_base);
    let filename = format!("{}{}", new_mime_id(None), extension);
    let dest = PathBuf::from("./uploads/basic").join(&filename);

    let write_file_res = write_file(&dest, &bytes).await.map_err(|e| {
        log::error!("disk write failed for `{}`: {}", filename, e);
        "could not save file".to_string()
    });

    if let Err(err) = write_file_res {
        log::error!("Error writing file using basic HTTP body handler `{}`: {}", filename, err);
        return Err(err);
    }

    log::info!("upload saved | file={} mime={} size={}B", filename, mime_base, bytes.len());

    let full_path = match tokio::fs::canonicalize(&dest).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("failed to resolve full path for `{}`: {}", filename, e);
            return Err("could not resolve file path".to_string());
        }
    };

    let res = axum::Json(json!(
        { "filePath": full_path }
    ));

    Ok(res)
}

pub async fn video_upload_multipart_handler(req: MultipartReq) -> Result<axum::Json<Value>, String> {
    let mime_base = req.file_content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Multipart Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err(format!("unsupported file type: {mime_base}"));
    }

    if req.file.is_empty() {
        log::error!("Multipart Handler >> upload rejected: empty file, file:{}, line:{}", file!(), line!());
        return Err("empty file".to_string());
    }

    // 2. Generate filename + write to disk
    let extension = mime_to_extension(&mime_base);
    let filename = format!("{}{}", new_mime_id(None), extension);
    let dest = PathBuf::from("./uploads/multipart").join(&filename);

    let write_file_res = write_file(&dest, &req.file).await.map_err(|e| {
        log::error!("Multipart Handler >> disk write failed for `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
        "could not save file".to_string()
    });

    if let Err(err) = write_file_res {
        log::error!("Multipart Handler >> Error writing file using multipart handler `{}`: {}, file:{}, line:{}", filename, err, file!(), line!());
        return Err(err);
    }

    log::info!("Multipart Handler >> upload saved | file={} mime={} size={}B", filename, mime_base, req.file.len());

    let full_path = match tokio::fs::canonicalize(&dest).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("Multipart Handler >> failed to resolve full path for `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
            return Err("could not resolve file path".to_string());
        }
    };

    let res = axum::Json(json!(
        { "filePath": full_path }
    ));

    Ok(res)
}

pub async fn video_upload_stream_handler(content_type: String, stream: &mut BodyDataStream) -> Result<axum::Json<Value>, String> {
    // 1. Validate MIME
    let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Stream Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err(format!("unsupported file type: {mime_base}"));
    }

    // 3. Generate filename
    let extension = mime_to_extension(&mime_base);
    let filename = format!("{}{}", new_mime_id(None), extension);
    let dest = PathBuf::from("./uploads/stream").join(&filename);

    // Create the file on disk to write the incoming stream to.
    let mut file = match tokio::fs::File::create(&dest).await {
        Ok(f) => f,
        Err(e) => {
            log::error!("Stream Handler >> failed to create file `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
            return Err("Error creating file".to_string());
        }
    };

    // 2. Write the incoming body stream to the file in chunks, without buffering the entire body into memory.
    let mut total_bytes: u64 = 0;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                total_bytes += bytes.len() as u64;

                log::info!("received chunk={} bytes, total={} MB", bytes.len(), total_bytes as f64 / 1024.0 / 1024.0);

                if let Err(err) = file.write_all(&bytes).await {
                    log::error!("Stream Handler >> Failed to write chunk: {err}, file:{}, line:{}", file!(), line!());
                    return Err("Error writing file chunk".to_string());
                }
                // sleep(Duration::from_secs(15)).await; // Simulate a delay for testing purposes
            }
            Err(err) => {
                log::error!("Stream Handler >> Read error: {err}, file:{}, line:{}", file!(), line!());
                return Err("Error reading stream".to_string());
            }
        }
    }

    log::info!("Stream Handler >> upload saved | file={} mime={} size={}B", filename, mime_base, total_bytes);

    let full_path = match tokio::fs::canonicalize(&dest).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("Stream Handler >> failed to resolve full path for `{}`: {}, file:{}, line:{}", filename, e, file!(), line!());
            return Err("Could not resolve file path".to_string());
        }
    };

    let res = axum::Json(json!(
        { "filePath": full_path }
    ));

    Ok(res)
}

pub async fn video_upload_multipart_stream_handler(multipart: Multipart) -> Result<axum::Json<Value>, String> {
    // 1. Validate MIME
    // let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    // if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
    //     log::error!("Multipart Stream Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
    //     return Err(format!("unsupported file type: {mime_base}"));
    // }

    // 2. Generate filename
    // let extension = mime_to_extension(&mime_base);
    // let filename = format!("{}{}", new_mime_id(None), extension);
    // let dest = PathBuf::from("./uploads/multipart_stream").join(&filename);

    match MultipartReq::stream_to_file(multipart).await {
        Ok(file_name) => {
            // log::info!("Multipart Stream Handler >> upload saved | file={} mime={}", filename, mime_base);

            let full_path = match tokio::fs::canonicalize(&file_name).await {
                Ok(path) => path,
                Err(e) => {
                    log::error!("Multipart Stream Handler >> failed to resolve full path for `{}`: {}, file:{}, line:{}", file_name, e, file!(), line!());
                    return Err("could not resolve file path".to_string());
                }
            };

            let res = axum::Json(json!(
                { "filePath": full_path }
            ));

            Ok(res)
        }
        Err(err) => {
            log::error!("Multipart Stream Handler >> Error writing file using multipart stream handler: {err}, file:{}, line:{}", file!(), line!());
            Err(err)
        }
    }
}

pub async fn video_upload_chunked_init_handler(req: ChunkedUploadInitRequest) -> Result<ChunkedUploadInitResponse, (StatusCode, String)> {
    // The client declares its chunk count up front; reject a nonsensical one
    // here so a transfer is never created with an impossible completion target.
    if req.total_chunks == 0 {
        log::error!("Chunked Init Handler >> rejected: total_chunks must be greater than 0, file:{}, line:{}", file!(), line!());
        return Err((StatusCode::BAD_REQUEST, "Total Chunks must be greater than 0".to_string()));
    }

    // The file type is a whole-file property, so validate and pin it here at
    // init. Every later chunk is checked against this extension. Stored without
    // the leading dot so chunk filenames read as `0.mp4`, not `0..mp4`.
    let mime_base = req.content_type.split(';').next().unwrap_or("").trim().to_lowercase();
    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Chunked Init Handler >> rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {mime_base}")));
    }
    let extension = mime_to_extension(&mime_base).trim_start_matches('.').to_string();

    let upload_id = generate_uuid(); // Generate a new transfer ID
    set_upload_state(upload_id.clone(), ChunkDetails { file_name: new_mime_id(None), total_chunks: req.total_chunks, received_chunks: std::collections::HashSet::new(), extension, chunk_type: ChunkType::Raw, transfer_id: upload_id.clone(), status: ProcessingStatus::None }).await; // Initialize the state
    Ok(ChunkedUploadInitResponse { upload_id })
}

pub async fn video_upload_chunked_handler(content_type: String, query: ChunkQuery, body: Bytes) -> Result<ChunkedUploadResponse, (StatusCode, String)> {
    // 1. Validate MIME
    let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Stream Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {mime_base}")));
    }

    // 2. Look up the transfer. `total_chunks` and `file_name` were both fixed at
    // init time and never change, so this snapshot is authoritative.
    let upload_id = &query.upload_id;
    let (file_name, total_chunks, extension) = match get_upload_state(upload_id).await {
        Some(details) => (details.file_name, details.total_chunks, details.extension),
        None => {
            log::error!("Chunked Upload Handler >> upload rejected: invalid upload ID `{}`, file:{}, line:{}", upload_id, file!(), line!());
            return Err((StatusCode::BAD_REQUEST, "Invalid upload ID".to_string()));
        }
    };

    // 3. This chunk's type must match the extension pinned at init. Guards
    // against a client sending mismatched Content-Types across chunks, which
    // would otherwise scatter files under different extensions and break
    // reassembly (the reassembler reads with a single extension).
    let chunk_extension = mime_to_extension(&mime_base).trim_start_matches('.');
    if chunk_extension != extension {
        log::error!("Chunked Upload Handler >> upload rejected: chunk type `{}` does not match upload type `.{}` for upload ID `{}`, file:{}, line:{}", mime_base, extension, upload_id, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Chunk type `{mime_base}` does not match upload type `.{extension}`")));
    }

    // 4. Validate the chunk index against the declared total. Indices are
    // 0-based, so a valid index is in `0..total_chunks`.
    if query.chunk_index >= total_chunks {
        log::error!("Chunked Upload Handler >> upload rejected: chunk_index {} out of range (total_chunks={}) for upload ID `{}`, file:{}, line:{}", query.chunk_index, total_chunks, upload_id, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, "Chunk Index out of range".to_string()));
    }

    // 5. Write the chunk to the file.
    let chunk_path = PathBuf::from(format!("./uploads/chunk/{}", file_name)).join(format!("{}.{extension}", query.chunk_index));

    if let Err(e) = write_file(&chunk_path, &body).await {
        log::error!("Chunked Upload Handler >> failed to write chunk: {}, file:{}, line:{}", e, file!(), line!());
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to write chunk".to_string()));
    }

    // 6. Record this chunk atomically under one lock hold. Inserting the index
    // into a set makes a retried chunk idempotent instead of double-counting.
    let received = update_upload_state(upload_id, |details| {
        details.received_chunks.insert(query.chunk_index);
        details.received_chunks.len() as u32
    })
    .await;

    match received {
        Some(received) => {
            log::info!("Chunked Upload Handler >> chunk saved | upload_id={} chunk_index={} received={}/{} size={}B", upload_id, query.chunk_index, received, total_chunks, body.len());
            Ok(ChunkedUploadResponse { upload_id: upload_id.clone(), chunk_index: query.chunk_index, message: "Chunk uploaded successfully".to_string() })
        }
        None => {
            log::error!("Chunked Upload Handler >> transfer `{}` no longer active; discarded chunk {}, file:{}, line:{}", upload_id, query.chunk_index, file!(), line!());
            Err((StatusCode::BAD_REQUEST, "Invalid upload ID".to_string()))
        }
    }
}

pub async fn video_upload_resumable_init_handler(req: ResumableUploadInitRequest) -> Result<ResumableUploadInitResponse, (StatusCode, String)> {
    // The client declares its chunk count up front; reject a nonsensical one
    // here so a transfer is never created with an impossible completion target.
    if req.total_chunks == 0 {
        log::error!("Resumable Init Handler >> rejected: total_chunks must be greater than 0, file:{}, line:{}", file!(), line!());
        return Err((StatusCode::BAD_REQUEST, "Total Chunks must be greater than 0".to_string()));
    }

    // The file type is a whole-file property, so validate and pin it here at
    // init. Every later chunk is checked against this extension. Stored without
    // the leading dot so chunk filenames read as `0.mp4`, not `0..mp4`.
    let mime_base = req.content_type.split(';').next().unwrap_or("").trim().to_lowercase();
    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Resumable Init Handler >> rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {mime_base}")));
    }
    let extension = mime_to_extension(&mime_base).trim_start_matches('.').to_string();

    let upload_id = generate_uuid(); // Generate a new transfer ID
    set_upload_state(upload_id.clone(), ChunkDetails { file_name: new_mime_id(None), total_chunks: req.total_chunks, received_chunks: std::collections::HashSet::new(), extension, chunk_type: ChunkType::Resumable, transfer_id: upload_id.clone(), status: ProcessingStatus::None }).await; // Initialize the state
    Ok(ResumableUploadInitResponse { upload_id })
}

// The endpoint that makes the flow actually resumable: the client asks which
// chunk indices the server already holds so it can send only the missing ones.
// Without this, an interrupted transfer has no choice but to restart from 0.
pub async fn video_upload_resumable_status_handler(query: ResumableStatusQuery) -> Result<ResumableStatusResponse, (StatusCode, String)> {
    let upload_id = &query.upload_id;
    match get_upload_state(upload_id).await {
        Some(details) => {
            // Guard against a client pointing a raw-chunked upload_id at the
            // resumable status endpoint — the two flows don't share progress.
            if details.chunk_type != ChunkType::Resumable {
                log::error!("Resumable Status Handler >> upload rejected: `{}` is not a resumable transfer, file:{}, line:{}", upload_id, file!(), line!());
                return Err((StatusCode::BAD_REQUEST, "Upload ID is not a resumable transfer".to_string()));
            }

            // Return indices sorted so the client can diff against 0..total
            // deterministically; `received_chunks` is a set, so these are the
            // distinct chunks actually persisted.
            let mut received_chunks: Vec<u32> = details.received_chunks.into_iter().collect();
            received_chunks.sort_unstable();
            let complete = received_chunks.len() as u32 >= details.total_chunks;

            log::info!("Resumable Status Handler >> upload_id={} received={}/{} complete={}", upload_id, received_chunks.len(), details.total_chunks, complete);
            Ok(ResumableStatusResponse { upload_id: upload_id.clone(), total_chunks: details.total_chunks, received_chunks, complete })
        }
        None => {
            // Unknown id: either it never existed, or it completed and the
            // scheduler already reassembled + dropped the state. The client
            // treats a 404 as "nothing to resume — start a fresh transfer".
            log::info!("Resumable Status Handler >> unknown or completed upload_id={}, file:{}, line:{}", upload_id, file!(), line!());
            Err((StatusCode::NOT_FOUND, "Unknown or completed upload ID".to_string()))
        }
    }
}

pub async fn video_upload_resumable_handler(content_type: String, query: ResumableChunkQuery, body: Bytes) -> Result<ResumableUploadResponse, (StatusCode, String)> {
    // 1. Validate MIME
    let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

    if !ALLOWED_MIME_TYPES.contains(&mime_base.as_str()) {
        log::error!("Stream Handler >> upload rejected: unsupported MIME type `{}`, file:{}, line:{}", mime_base, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Unsupported file type: {mime_base}")));
    }

    // 2. Look up the transfer. `total_chunks` and `file_name` were both fixed at
    // init time and never change, so this snapshot is authoritative.
    let upload_id = &query.upload_id;
    let (file_name, total_chunks, extension) = match get_upload_state(upload_id).await {
        Some(details) => (details.file_name, details.total_chunks, details.extension),
        None => {
            log::error!("Resumable Upload Handler >> upload rejected: invalid upload ID `{}`, file:{}, line:{}", upload_id, file!(), line!());
            return Err((StatusCode::BAD_REQUEST, "Invalid upload ID".to_string()));
        }
    };

    // 3. This chunk's type must match the extension pinned at init. Guards
    // against a client sending mismatched Content-Types across chunks, which
    // would otherwise scatter files under different extensions and break
    // reassembly (the reassembler reads with a single extension).
    let chunk_extension = mime_to_extension(&mime_base).trim_start_matches('.');
    if chunk_extension != extension {
        log::error!("Resumable Upload Handler >> upload rejected: chunk type `{}` does not match upload type `.{}` for upload ID `{}`, file:{}, line:{}", mime_base, extension, upload_id, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, format!("Chunk type `{mime_base}` does not match upload type `.{extension}`")));
    }

    // 4. Validate the chunk index against the declared total. Indices are
    // 0-based, so a valid index is in `0..total_chunks`.
    if query.chunk_index >= total_chunks {
        log::error!("Resumable Upload Handler >> upload rejected: chunk_index {} out of range (total_chunks={}) for upload ID `{}`, file:{}, line:{}", query.chunk_index, total_chunks, upload_id, file!(), line!());
        return Err((StatusCode::BAD_REQUEST, "Chunk Index out of range".to_string()));
    }

    // 5. Write the chunk to the file.
    let chunk_path = PathBuf::from(format!("./uploads/resumable/{}", file_name)).join(format!("{}.{extension}", query.chunk_index));

    if let Err(e) = write_file(&chunk_path, &body).await {
        log::error!("Resumable Upload Handler >> failed to write chunk: {}, file:{}, line:{}", e, file!(), line!());
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to write chunk".to_string()));
    }

    // 6. Record this chunk atomically under one lock hold. Inserting the index
    // into a set makes a retried chunk idempotent instead of double-counting.
    let received = update_upload_state(upload_id, |details| {
        details.received_chunks.insert(query.chunk_index);
        details.received_chunks.len() as u32
    })
    .await;

    match received {
        Some(received) => {
            log::info!("Resumable Upload Handler >> chunk saved | upload_id={} chunk_index={} received={}/{} size={}B", upload_id, query.chunk_index, received, total_chunks, body.len());
            Ok(ResumableUploadResponse { upload_id: upload_id.clone(), chunk_index: query.chunk_index, message: "Chunk uploaded successfully".to_string() })
        }
        None => {
            log::error!("Resumable Upload Handler >> transfer `{}` no longer active; discarded chunk {}, file:{}, line:{}", upload_id, query.chunk_index, file!(), line!());
            Err((StatusCode::BAD_REQUEST, "Invalid upload ID".to_string()))
        }
    }
}

// pub async fn video_upload_chunked_waiting_handler(req: ChunkedUploadRequest) -> Result<ChunkedUploadResponse, (StatusCode, String)> {
//     // Check if already in process of uploading, if so
//     // if already in process take chunk and append to file, if not create new file and start process

//     let extension = req.filename.split('.').last().unwrap_or("");

//     if extension.is_empty() || !ALLOWED_EXTENSIONS.contains(&extension) {
//         log::error!("Chunked Upload Handler >> upload rejected: missing or unsupported file extension, file:{}, line:{}", file!(), line!());
//         return Err((StatusCode::BAD_REQUEST, "Missing or unsupported file extension".to_string()));
//     }

//     let (upload ID, file_name) = if let Some(upload ID) = req.upload ID {
//         let file_name = get_upload_state(&upload ID).await;
//         if file_name.is_none() {
//             log::error!("Chunked Upload Handler >> upload rejected: invalid upload ID `{}`, file:{}, line:{}", upload ID, file!(), line!());
//             return Err((StatusCode::BAD_REQUEST, "Invalid upload ID".to_string()));
//         }
//         (upload ID, file_name.unwrap())
//     } else {
//         let file_name = format!("{}.{}", new_mime_id(None), extension);
//         let upload ID = generate_uuid(); // Generate a new transfer ID
//         set_upload_state(upload ID.clone(), file_name.clone()).await;
//         (upload ID, file_name)
//     };

//     // decode from base 64 to bytes
//     let decoded_data = match base64::decode(&req.data) {
//         Ok(data) => data,
//         Err(e) => {
//             log::error!("Chunked Upload Handler >> failed to decode base64 data: {}, file:{}, line:{}", e, file!(), line!());
//             return Err((StatusCode::BAD_REQUEST, "Failed to decode base64 data".to_string()));
//         }
//     };

//     match storage::append_file(&PathBuf::from("./uploads/chunked_waiting").join(&file_name), &decoded_data).await {
//         Ok(_) => (),
//         Err(e) => {
//             log::error!("Chunked Upload Handler >> failed to write file: {}, file:{}, line:{}", e, file!(), line!());
//             return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to write file".to_string()));
//         }
//     }

//     Ok(ChunkedUploadResponse { upload ID, message: "Chunk uploaded successfully".to_string() })
// }
