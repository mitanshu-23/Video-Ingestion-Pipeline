use axum::{
    Router,
    routing::{get, post},
};

pub mod stream_model;
pub mod stream_service;
pub mod stream_struct;

pub fn stream_routes() -> Router {
    Router::new()
        .route("/video/basic/stream/v1", post(stream_service::stream_basic_svc))
        .route("/video/chunk/stream/v1", post(stream_service::stream_chunk_svc))
        .route("/video/rangereq/init/stream/v1", post(stream_service::stream_range_request_init))
        // GET so a native <video src=...> can drive it — media elements only
        // issue bodyless GETs, and that's what lets the BROWSER manage Range
        // requests on demand (fetch-ahead, backpressure, seek). POST is kept so
        // the range_stream.html single-range probe still works against it.
        .route("/video/rangereq/stream/v1", get(stream_service::stream_range_request).post(stream_service::stream_range_request))
}
