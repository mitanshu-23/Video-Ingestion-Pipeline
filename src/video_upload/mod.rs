use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
mod video_model;
mod video_service;
pub mod video_struct;

// The `Multipart` extractor enforces axum's default 2 MB request body limit,
// which truncates real video uploads and makes multer fail with
// "Error parsing `multipart/form-data` request". The raw-`Body` basic/stream
// endpoints are not bound by this limit, so they need no layer.
const _MAX_MULTIPART_BYTES: usize = 50 * 1024 * 1024;

pub fn video_routes() -> Router {
    Router::new()
        .route("/video/basic/upload/v1", post(video_service::upload_basic_http_body_svc))
        // Buffers the whole file in memory, so keep a sane 50 MB cap.
        .route("/video/multipart/upload/v1", post(video_service::upload_multipart_svc).layer(DefaultBodyLimit::disable()))
        .route("/video/stream/upload/v1", post(video_service::upload_stream_svc))
        // Streams straight to disk, so lift the limit entirely.
        .route("/video/multipart/stream/upload/v1", post(video_service::upload_multipart_stream_svc).layer(DefaultBodyLimit::disable()))
        // Each chunk arrives as a base64 JSON body, which inflates the raw
        // bytes by ~33% and would otherwise trip the default 2 MB limit.
        .route("/video/chunked/init/upload/v1", post(video_service::upload_chunked_init_svc).layer(DefaultBodyLimit::disable()))
        .route("/video/chunked/upload", post(video_service::upload_chunked_svc).layer(DefaultBodyLimit::disable()))
        .route("/video/resumable/init/upload/v1", post(video_service::upload_resumable_init_svc).layer(DefaultBodyLimit::disable()))
        // Progress query the client hits before (re)sending chunks so an
        // interrupted upload resumes from the missing chunks instead of chunk 0.
        .route("/video/resumable/status/v1", get(video_service::upload_resumable_status_svc))
        .route("/video/resumable/upload/v1", post(video_service::upload_resumable_svc).layer(DefaultBodyLimit::disable()))
}
