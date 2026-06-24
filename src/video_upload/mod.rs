use axum::Router;
mod video_model;
mod video_service;
mod video_struct;

pub fn video_routes() -> Router {
    Router::new().route("/video/basic/upload/v1", axum::routing::post(video_service::upload_basic_http_body_svc)).route("/video/multipart/upload/v1", axum::routing::post(video_service::upload_multipart_svc))
}
