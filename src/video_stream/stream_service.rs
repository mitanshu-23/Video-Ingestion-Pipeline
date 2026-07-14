use crate::video_stream::{stream_model, stream_struct::VideoStreamRequest};
use axum::Json;
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

pub async fn stream_basic_svc(Query(payload): Query<VideoStreamRequest>) -> Response {
    log::info!("/video/basic/stream called");
    match stream_model::basic_video_stream(payload).await {
        // Send the file's raw bytes as the body with the right Content-Type, so
        // the browser renders it inline (<img>/<video> src) with no JSON
        // envelope to serialize/parse. See stream_model for the why.
        Ok((content_type, data)) => (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response(),
        // Propagate the model's own status code (e.g. 400 for an invalid file)
        // instead of collapsing everything to 500.
        Err(err) => err.into_response(),
        // --- OLD APPROACH (JSON envelope) — kept for reference ---
        // Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        // Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}
//

pub async fn stream_chunk_svc(Query(payload): Query<VideoStreamRequest>) -> Response {
    log::info!("/video/chunk/stream called");
    match stream_model::chunk_video_stream(payload).await {
        // Send the file's raw bytes as the body with the right Content-Type, so
        // the browser renders it inline (<img>/<video> src) with no JSON
        // envelope to serialize/parse. See stream_model for the why.
        Ok((content_type, data)) => (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response(),
        // Propagate the model's own status code (e.g. 400 for an invalid file)
        // instead of collapsing everything to 500.
        Err(err) => err.into_response(),
        // --- OLD APPROACH (JSON envelope) — kept for reference ---
        // Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        // Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

// Range flow uses `Query` (not `Json`) because the actual byte-serving endpoint
// is loaded straight into a `<video src=...>`, which can only issue GETs with no
// body — so the file identity (fileName, uploadType) must ride in the query
// string. The init request mirrors that shape so both halves share one URL form.
pub async fn stream_range_request_init(Query(req): Query<VideoStreamRequest>) -> Response {
    log::info!("/video/rangereq/init/stream/v1 called");
    match stream_model::http_range_init_request(req).await {
        Ok(res) => {
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", res.content_type.parse().unwrap());
            // NOT `Content-Length`: that's a framing header hyper computes from
            // the actual body (here just "OK"), and a manual value that disagrees
            // is rejected. Advertise the file's real size in a custom header the
            // client reads to plan its Range windows.
            headers.insert("X-Content-Length", res.content_length.into());
            headers.insert("Accept-Ranges", "bytes".parse().unwrap());

            // Init is a metadata probe, not partial content → 200, not 206.
            (StatusCode::OK, headers).into_response()
        }
        Err(err) => err.into_response(),
    }
}

pub async fn stream_range_request(headers: HeaderMap, Query(req): Query<VideoStreamRequest>) -> Response {
    log::info!("/video/rangereq/stream/v1 called");
    // Hand the raw Range header to the model, which parses + clamps it against
    // the real file size (a malformed / open-ended / oversized range can't panic
    // or over-read here). See stream_model::parse_range.
    let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    match stream_model::http_range_request(&range_header, req).await {
        Ok((body, start, end, total, content_type)) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
            headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            // A 206 MUST advertise which slice of the whole it carries.
            headers.insert(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, total).parse().unwrap());
            (StatusCode::PARTIAL_CONTENT, headers, body).into_response()
        }
        Err(err) => err.into_response(),
    }
}
