mod healthcheck;
mod routes;
mod utils;
mod video_upload;
mod video_stream;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // Initialize logger
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    log::info!("Welcome to the Video Ingestion Pipeline! v1.0.0");

    // Start Axum Server here
    let app = routes::get_all_routes();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    log::info!("Starting server at http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}