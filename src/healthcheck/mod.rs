use axum::Router;

pub fn health_check_routes() -> Router {
    Router::new().route("/health/check", axum::routing::get(|| async { "OK" }))
}
