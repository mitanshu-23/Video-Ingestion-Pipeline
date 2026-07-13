use axum::{Router, middleware};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    healthcheck::health_check_routes,
    prometheus::{HTTP_REQUEST_DURATION, HTTP_REQUESTS_TOTAL, get_metrics_route},
    video_stream::stream_routes,
    video_upload::video_routes,
};

pub fn get_all_routes() -> Router {
    // Permissive CORS so the browser-based upload tester can reach the API
    // from any origin (file://, a static server, etc.). Tighten allow_origin
    // to specific hosts before this goes anywhere near production.
    // expose_headers(Any) is required so browser JS can READ non-safelisted
    // response headers cross-origin (e.g. the range init's X-Content-Length).
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any).expose_headers(Any);

    Router::new().merge(health_check_routes()).merge(video_routes()).merge(get_metrics_route()).merge(stream_routes()).layer(middleware::from_fn(metrics_middleware)).layer(cors)
}

async fn metrics_middleware(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let start = std::time::Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    HTTP_REQUEST_DURATION.with_label_values(&[&method, &path, &status]).observe(elapsed);
    HTTP_REQUESTS_TOTAL.with_label_values(&[&method, &path, &status]).inc();

    response
}
