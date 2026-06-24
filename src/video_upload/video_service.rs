use axum::{
    body::Body,
    extract::Multipart,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use crate::video_upload::{video_model, video_struct::MultipartReq};

pub async fn upload_basic_http_body_svc(headers: HeaderMap, body: Body) -> Response {
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
