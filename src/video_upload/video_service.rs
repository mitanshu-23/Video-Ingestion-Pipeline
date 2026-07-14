use crate::video_upload::{
    video_model,
    video_struct::{ChunkQuery, ChunkedUploadInitRequest, MultipartReq, ResumableChunkQuery, ResumableStatusQuery, ResumableUploadInitRequest},
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

pub async fn upload_basic_http_body_svc(headers: HeaderMap, body: Body) -> Response {
    log::info!("/video/basic/upload/v1 called");
    // 1. Extract Content-Type
    let content_type = match headers.get("content-type").and_then(|v| v.to_str().ok()) {
        Some(ct) => ct.to_string(),
        None => {
            log::error!("upload rejected: missing Content-Type header");
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "missing Content-Type header").into_response();
        }
    };

    // 2. Content-Length pre-check
    let max = 50 * 1024 * 1024_usize;

    if let Some(val) = headers.get("content-length") {
        let claimed: usize = val.to_str().unwrap_or("0").parse().unwrap_or(0);
        if claimed > max {
            log::error!("upload rejected: Content-Length {} exceeds limit {}", claimed, max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "file exceeds size limit").into_response();
        }
    }

    // 3. Buffer body
    // Loads the entire body into memory.
    let bytes = match axum::body::to_bytes(body, max).await {
        Ok(b) => b,
        Err(_) => {
            log::error!("upload rejected: body exceeded size limit of {} bytes", max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "body exceeded size limit").into_response();
        }
    };

    match video_model::video_upload_basic_http_body_handler(bytes, content_type).await {
        Ok(res) => (StatusCode::OK, res).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

// Expect body to be multipart/form-data with exactly `file` and `name` fields.
pub async fn upload_multipart_svc(_headers: HeaderMap, multipart: Multipart) -> Response {
    log::info!("/video/multipart/upload/v1 called");
    // Parse the raw multipart stream into our typed request structure.
    let req = match MultipartReq::from_multipart(multipart).await {
        Ok(req) => req,
        Err(err) => {
            log::error!("multipart upload rejected: {err}");
            return (StatusCode::BAD_REQUEST, err).into_response();
        }
    };

    match video_model::video_upload_multipart_handler(req).await {
        Ok(res) => (StatusCode::OK, res).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

pub async fn upload_stream_svc(headers: HeaderMap, body: Body) -> Response {
    log::info!("/video/stream/upload/v1 called");
    // 1. Extract Content-Type
    let content_type = match headers.get("content-type").and_then(|v| v.to_str().ok()) {
        Some(ct) => ct.to_string(),
        None => {
            log::error!("upload rejected: missing Content-Type header");
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "missing Content-Type header").into_response();
        }
    };

    // 2. Content-Length pre-check
    // let max = 50 * 1024 * 1024_usize;

    // if let Some(val) = headers.get("content-length") {
    //     let claimed: usize = val.to_str().unwrap_or("0").parse().unwrap_or(0);
    //     if claimed > max {
    //         log::error!("upload rejected: Content-Length {} exceeds limit {}", claimed, max);
    //         return (StatusCode::PAYLOAD_TOO_LARGE, "file exceeds size limit").into_response();
    //     }
    // }

    // 3. Stream body
    // This is a streaming upload, so we don't buffer the entire body into memory. Body is read in chunks and written to a file on disk.
    // Frontend sends data as whole but browser sends it in chunks, so we can read the body as a stream and write to disk as we go.
    let mut stream = body.into_data_stream();

    match video_model::video_upload_stream_handler(content_type, &mut stream).await {
        Ok(res) => (StatusCode::OK, res).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

// Expect body to be multipart/form-data with exactly `file` and `name` fields.
pub async fn upload_multipart_stream_svc(_headers: HeaderMap, multipart: Multipart) -> Response {
    log::info!("/video/multipart/stream/upload/v1 called");
    match video_model::video_upload_multipart_stream_handler(multipart).await {
        Ok(res) => (StatusCode::OK, res).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

pub async fn upload_chunked_init_svc(Json(req): Json<ChunkedUploadInitRequest>) -> Response {
    log::info!("/video/chunked/init/upload/v1 called");
    match video_model::video_upload_chunked_init_handler(req).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn upload_chunked_svc(Query(query): Query<ChunkQuery>, headers: HeaderMap, body: Body) -> Response {
    log::info!("/video/chunked/upload called with query params: {:?}", query);
    // 1. Extract Content-Type
    let content_type = match headers.get("content-type").and_then(|v| v.to_str().ok()) {
        Some(ct) => ct.to_string(),
        None => {
            log::error!("upload rejected: missing Content-Type header");
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "missing Content-Type header").into_response();
        }
    };

    // 2. Content-Length pre-check
    let max = 50 * 1024 * 1024_usize;

    if let Some(val) = headers.get("content-length") {
        let claimed: usize = val.to_str().unwrap_or("0").parse().unwrap_or(0);
        if claimed > max {
            log::error!("upload rejected: Content-Length {} exceeds limit {}", claimed, max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "file exceeds size limit").into_response();
        }
    }

    // 3. Buffer body
    // Loads the entire body into memory.
    let bytes = match axum::body::to_bytes(body, max).await {
        Ok(b) => b,
        Err(_) => {
            log::error!("upload rejected: body exceeded size limit of {} bytes", max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "body exceeded size limit").into_response();
        }
    };

    match video_model::video_upload_chunked_handler(content_type, query, bytes).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn upload_resumable_init_svc(Json(req): Json<ResumableUploadInitRequest>) -> Response {
    log::info!("/video/resumable/init/upload/v1 called");
    match video_model::video_upload_resumable_init_handler(req).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn upload_resumable_status_svc(Query(query): Query<ResumableStatusQuery>) -> Response {
    log::info!("/video/resumable/status/v1 called with query params: {:?}", query);
    match video_model::video_upload_resumable_status_handler(query).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn upload_resumable_svc(Query(query): Query<ResumableChunkQuery>, headers: HeaderMap, body: Body) -> Response {
    log::info!("/video/resumable/upload/v1 called with query params: {:?}", query);
    // 1. Extract Content-Type
    let content_type = match headers.get("content-type").and_then(|v| v.to_str().ok()) {
        Some(ct) => ct.to_string(),
        None => {
            log::error!("upload rejected: missing Content-Type header");
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "missing Content-Type header").into_response();
        }
    };

    // 2. Content-Length pre-check
    let max = 50 * 1024 * 1024_usize;

    if let Some(val) = headers.get("content-length") {
        let claimed: usize = val.to_str().unwrap_or("0").parse().unwrap_or(0);
        if claimed > max {
            log::error!("upload rejected: Content-Length {} exceeds limit {}", claimed, max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "file exceeds size limit").into_response();
        }
    }

    // 3. Buffer body
    // Loads the entire body into memory.
    let bytes = match axum::body::to_bytes(body, max).await {
        Ok(b) => b,
        Err(_) => {
            log::error!("upload rejected: body exceeded size limit of {} bytes", max);
            return (StatusCode::PAYLOAD_TOO_LARGE, "body exceeded size limit").into_response();
        }
    };

    match video_model::video_upload_resumable_handler(content_type, query, bytes).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(err) => err.into_response(),
    }
}
