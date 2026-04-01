use rustls::crypto::aws_lc_rs;
use std::thread::park;

mod entity;
mod login;
mod register;
mod textchat;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc-rs crypto provider");

    tokio::spawn(async move {
        if let Err(e) = register::run().await {
            dbg!(e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = login::run().await {
            dbg!(e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = textchat::run().await {
            dbg!(e);
        }
    });
    park();
}
