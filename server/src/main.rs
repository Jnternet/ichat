use anyhow::Result;
use rustls::crypto::aws_lc_rs;
use server::axum;
use server::textchat;
use server::voice_chat;
use shared::log::init_log;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");
    let _g = init_log("server", "server");
    info!("Server starting...");

    tokio::spawn(async {
        match axum::run_https_server().await {
            Ok(_) => info!("HTTPS server stopped gracefully"),
            Err(e) => error!("HTTPS server error: {:?}", e),
        }
    });

    tokio::spawn(async {
        match textchat::run().await {
            Ok(_) => info!("Text chat server stopped gracefully"),
            Err(e) => error!("Text chat server error: {:?}", e),
        }
    });

    tokio::spawn(async {
        match voice_chat::run().await {
            Ok(_) => info!("Voice chat server stopped gracefully"),
            Err(e) => error!("Voice chat server error: {:?}", e),
        }
    });

    info!("All services started successfully");
    tokio::signal::ctrl_c().await?;
    info!("Server shutting down...");
    Ok(())
}
