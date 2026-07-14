mod chunk_scheduler;
mod healthcheck;
mod prometheus;
mod routes;
mod utils;
mod video_upload;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // Initialize logger
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    // Initialize Prometheus metrics
    // Note: the `process` feature of the `prometheus` crate auto-registers
    // the ProcessCollector with the default registry, so no manual call is needed.

    log::info!("Welcome to the Video Ingestion Pipeline! v1.0.0");

    tokio::spawn(async {
        log::info!("Starting chunk receiver...");
        chunk_scheduler::channel::chunk_receiver().await;
    });

    tokio::spawn(async {
        log::info!("Starting resumable chunk receiver...");
        chunk_scheduler::channel::resumable_chunk_receiver().await;
    });

    tokio::spawn(async {
        log::info!("Starting chunk scheduler...");
        chunk_scheduler::scheduler::chunk_scheduler().await;
    });

    // Start Axum Server here
    let app = routes::get_all_routes();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    log::info!("Starting server at http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
