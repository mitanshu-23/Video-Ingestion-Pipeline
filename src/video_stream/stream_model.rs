use std::{io::SeekFrom, path::Path};

use axum::{body::Body, http::StatusCode};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::{
    utils::{
        common::{content_type_from_filename, extension_to_header, is_video_file},
        storage::read_file,
    },
    video_stream::stream_struct::{HHTPRangeReqInitResponse, VideoStreamRequest},
};

// Parses a single-range `Range: bytes=...` header against the known total size,
// supporting `start-end`, open-ended `start-` and suffix `-N` forms. Clamps the
// end to the last byte and returns 416 Range Not Satisfiable for out-of-bounds
// or inverted ranges, 400 for a malformed header — and never panics.
fn parse_range(range_header: &str, total: u64) -> Result<(u64, u64), (StatusCode, String)> {
    let malformed = || (StatusCode::BAD_REQUEST, "Malformed Range header".to_string());
    let unsatisfiable = || (StatusCode::RANGE_NOT_SATISFIABLE, "Requested range not satisfiable".to_string());

    let spec = range_header.trim().strip_prefix("bytes=").ok_or_else(malformed)?;
    // Only the first range of a multi-range request is served.
    let spec = spec.split(',').next().unwrap_or("").trim();
    let (s, e) = spec.split_once('-').ok_or_else(malformed)?;

    let (start, end) = if s.is_empty() {
        // Suffix range `bytes=-N` → the last N bytes.
        let n: u64 = e.parse().map_err(|_| malformed())?;
        if n == 0 {
            return Err(unsatisfiable());
        }
        (total.saturating_sub(n), total.saturating_sub(1))
    } else {
        let start: u64 = s.parse().map_err(|_| malformed())?;
        // Empty end (`start-`) means "to EOF".
        let end = if e.is_empty() { total.saturating_sub(1) } else { e.parse().map_err(|_| malformed())? };
        (start, end)
    };

    // Clamp the end to the final byte so an over-long request can't over-read
    // (read_exact EOF) or over-allocate the buffer.
    let end = end.min(total.saturating_sub(1));
    if total == 0 || start > end || start >= total {
        return Err(unsatisfiable());
    }
    Ok((start, end))
}

// Returns the file's raw bytes plus the `Content-Type` the service should serve
// them with. The caller streams the bytes back as the HTTP body verbatim.
//
// WHY RAW BYTES INSTEAD OF JSON (see the commented block below):
// The first version returned `VideoStreamResponse { file_name, data: Vec<u8> }`
// as JSON. serde serializes a `Vec<u8>` as a JSON array of numbers
// (`[137,80,78,...]`), which inflates the payload ~3-4x (each byte becomes 1-4
// digits + a comma) and costs a full serialize pass on the server and a
// `JSON.parse` + `Uint8Array.from` pass on the client. A 40 MB file became
// ~120-160 MB of text and took 6-7s to display. Returning the raw body sends
// exactly the file's bytes with zero serialization, so it's back to ~40 MB.
//
// NOTE: this still reads the whole file into memory and sends it in one shot —
// no HTTP Range / partial-content support yet, so video isn't seekable and
// playback can't start until the full body arrives. Range support is a TODO
// (206 Partial Content keyed off the `Range` request header).
pub async fn basic_video_stream(payload: VideoStreamRequest) -> Result<(&'static str, Vec<u8>), (StatusCode, String)> {
    let folder_path = Path::new("./uploads").join(payload.upload_type.clone());
    let file_path = if ["chunk", "resumable"].contains(&payload.upload_type.as_str()) { folder_path.join(format!("final_data")).join(payload.file_name.clone()) } else { folder_path.join(payload.file_name.clone()) };

    let file_data = match read_file(&file_path).await {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                log::error!("Video Stream - Basic Video Stream - User Requested with invalid file path, file_name: {}, upload_type: {}, file:{}, line: {}", payload.file_name, payload.upload_type, file!(), line!());
                return Err((StatusCode::BAD_REQUEST, format!("Invalid File Request")));
            }

            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Internal Server Error")));
        }
    };

    let content_type = content_type_from_filename(&payload.file_name);
    Ok((content_type, file_data))

    // --- OLD APPROACH (JSON byte array) — kept for reference, see WHY above ---
    // Returned the bytes wrapped in a JSON envelope. Simple, but the Vec<u8>
    // serializes to a giant `[..]` number array — slow to build, ~3-4x on the
    // wire, and slow to parse on the client.
    //
    // Ok(VideoStreamResponse { file_name: payload.file_name, data: file_data })
}

pub async fn chunk_video_stream(payload: VideoStreamRequest) -> Result<(&'static str, Body), (StatusCode, String)> {
    let folder_path = Path::new("./uploads").join(payload.upload_type.clone());
    let file_path = if ["chunk", "resumable"].contains(&payload.upload_type.as_str()) { folder_path.join(format!("final_data")).join(payload.file_name.clone()) } else { folder_path.join(payload.file_name.clone()) };

    let file = match tokio::fs::File::open(file_path.clone()).await {
        Ok(file) => file,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                log::error!("Video Stream - Basic Video Stream - User Requested with invalid file path, file_path: {:?}, file_name: {}, upload_type: {}, file:{}, line: {}", file_path, payload.file_name, payload.upload_type, file!(), line!());
                return Err((StatusCode::BAD_REQUEST, format!("Invalid File Request")));
            }

            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Internal Server Error")));
        }
    };

    // ReaderStream turns the AsyncRead file into a Stream<Item = Result<Bytes, _>>.
    // Body::from_stream produces a body of UNKNOWN length → hyper emits
    // `Transfer-Encoding: chunked` automatically. Default read buffer is small;
    // bump it so you get fewer, larger chunks off disk
    let stream = ReaderStream::with_capacity(file, 64 * 1024);
    let body = Body::from_stream(stream);

    Ok((content_type_from_filename(&payload.file_name), body))
}

pub async fn http_range_init_request(payload: VideoStreamRequest) -> Result<HHTPRangeReqInitResponse, (StatusCode, String)> {
    let folder_path = Path::new("./uploads").join(payload.upload_type.clone());
    let file_path = if ["chunk", "resumable"].contains(&payload.upload_type.as_str()) { folder_path.join(format!("final_data")).join(payload.file_name.clone()) } else { folder_path.join(payload.file_name.clone()) };

    if file_path.exists() {
        let is_video = match is_video_file(&file_path) {
            Ok(video_file) => video_file,
            Err(err) => {
                log::error!("Error checking Is Video File, error: {}, file:{}, line:{}", err, file!(), line!());
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()));
            }
        };

        if is_video {
            let metadata = std::fs::metadata(&file_path).unwrap();
            let content_length = metadata.len();

            let extension = &file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();

            return Ok(HHTPRangeReqInitResponse { content_length, content_type: extension_to_header(extension).to_owned() });
        } else {
            return Err((StatusCode::BAD_REQUEST, "Requested file is not a video file".to_string()));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, "Invalid file Request".to_string()));
    }
}

// Serves one Range window. Returns the sliced bytes plus (start, end, total) so
// the caller can build a spec-correct `Content-Range: bytes start-end/total`
// header on its 206 response. The raw Range header is parsed and clamped here
// (see parse_range) against the file's real size, so an out-of-bounds or
// malformed range yields 416/400 instead of a panic or a 500.
pub async fn http_range_request(range_header: &str, payload: VideoStreamRequest) -> Result<(Body, u64, u64, u64, &'static str), (StatusCode, String)> {
    let folder_path = Path::new("./uploads").join(payload.upload_type.clone());
    let file_path = if ["chunk", "resumable"].contains(&payload.upload_type.as_str()) { folder_path.join(format!("final_data")).join(payload.file_name.clone()) } else { folder_path.join(payload.file_name.clone()) };

    if file_path.exists() {
        let is_video = match is_video_file(&file_path) {
            Ok(video_file) => video_file,
            Err(err) => {
                log::error!("Error checking Is Video File, error: {}, file:{}, line:{}", err, file!(), line!());
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()));
            }
        };

        if is_video {
            let mut file = match tokio::fs::File::open(&file_path).await {
                Ok(file) => file,
                Err(err) => {
                    log::error!("Stream - http_range_request - Error :{:?} opening file at path: {:?}", err, file_path);
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error opening file".to_string()));
                }
            };

            // Total size drives both the Content-Range denominator and the range
            // clamping in parse_range.
            let total = match file.metadata().await {
                Ok(meta) => meta.len(),
                Err(err) => {
                    log::error!("Stream - http_range_request - Error :{:?} reading metadata for file at path: {:?}", err, file_path);
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error reading file metadata".to_string()));
                }
            };

            let (start_bytes, requested_end) = parse_range(range_header, total)?;

            // Cap how much we serve per response. The client (browser) chooses the
            // START via the Range header; the server bounds the LENGTH so an
            // open-ended `bytes=0-` can't read the whole file into memory. Because
            // the 206's `Content-Range` below still reports the true `total`, the
            // browser knows there's more and comes back for the next window as its
            // buffer drains — that's what makes playback stream on demand instead
            // of downloading everything up front (and lets you watch the windowed
            // 206s in the Network tab).
            const MAX_WINDOW: u64 = 1024 * 1024; // 1 MiB served per request
            let end_bytes = requested_end.min(start_bytes.saturating_add(MAX_WINDOW - 1));
            let length = end_bytes - start_bytes + 1;

            // Seek to the window start, then stream ONLY `length` bytes straight
            // from disk into the response body. `.take(length)` bounds the reader
            // so it stops at the window end, and ReaderStream + Body::from_stream
            // pipe the bytes through in 64 KB pieces — the whole window never sits
            // in one buffer (contrast the OLD APPROACH below).
            if let Err(err) = file.seek(SeekFrom::Start(start_bytes)).await {
                log::error!("Error: {:?} in seek function for file at path: {:?}", err, file_path);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error in file seek operation".to_string()));
            }

            let stream = ReaderStream::with_capacity(file.take(length), 64 * 1024);
            let body = Body::from_stream(stream);
            let content_type = content_type_from_filename(&payload.file_name);
            return Ok((body, start_bytes, end_bytes, total, content_type));

            // --- OLD APPROACH (buffer the whole window in memory) — kept for reference ---
            // Used a sync `std::fs::File`, seeked, then read_exact'd the entire
            // window into a `vec![0; length]` before handing it back as the body.
            // Correct, but the full window sits in RAM at once and the return type
            // was `Vec<u8>`. Streaming from disk (above) keeps memory flat regardless
            // of window size. The old body:
            //
            // match file.seek(std::io::SeekFrom::Start(start_bytes)) {
            //     Ok(_b) => {
            //         let mut buffer = vec![0; length as usize];
            //         if let Err(err) = file.read_exact(&mut buffer) {
            //             log::error!("Stream - http_range_request - Error :{:?} Error reading file at path :{:?}", err, file_path);
            //             return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error reading file".to_string()));
            //         }
            //         let content_type = content_type_from_filename(&payload.file_name);
            //         return Ok((buffer, start_bytes, end_bytes, total, content_type));
            //     }
            //     Err(err) => {
            //         log::error!("Error: {:?} in seek function for file at path: {:?}", err, file_path);
            //         return Err((StatusCode::INTERNAL_SERVER_ERROR, "Error in file seek operation".to_string()));
            //     }
            // }
        } else {
            return Err((StatusCode::BAD_REQUEST, "Requested file is not a video file".to_string()));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, "Invalid file Request".to_string()));
    }
}
