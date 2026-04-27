use axum::Router;

use crate::{healthcheck::health_check_routes, video_upload::video_routes};

pub fn get_all_routes() -> Router {
    Router::new().merge(health_check_routes()).merge(video_routes())
}
