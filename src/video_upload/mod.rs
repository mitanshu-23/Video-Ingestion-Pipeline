use axum::Router;
mod video_model;
mod video_service;
mod video_struct;

pub fn video_routes() -> Router {
    Router::new().route("/video/upload", axum::routing::post(video_service::upload_video_svc))
}
