use axum::Router;
use once_cell::sync::Lazy;
use prometheus::{Encoder, HistogramVec, IntCounterVec, register_histogram_vec, register_int_counter_vec};

pub static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| register_histogram_vec!("http_request_duration_seconds", "The HTTP request latencies in seconds.", &["method", "path", "endpoint"]).unwrap());

pub static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| register_int_counter_vec!("http_requests_total", "Total HTTP requests", &["method", "path", "status"]).unwrap());

pub fn get_metrics_route() -> Router {
    Router::new().route("/metrics", axum::routing::get(get_metrics))
}

async fn get_metrics() -> String {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();

    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    String::from_utf8(buffer).unwrap()
}

pub fn _register() {
    let pc = prometheus::process_collector::ProcessCollector::for_self();
    prometheus::default_registry().register(Box::new(pc)).unwrap();
}
