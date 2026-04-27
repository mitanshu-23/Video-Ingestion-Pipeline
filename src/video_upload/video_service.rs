use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::video_upload::video_model;

pub async fn upload_video_svc() -> Response {
    log::info!("Requested /video/upload endpoint");
    match video_model::video_upload_handler().await {
        Ok(res) => (StatusCode::OK, res).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}
