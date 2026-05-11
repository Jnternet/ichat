use anyhow::Result;
use rustls::crypto::aws_lc_rs;
use server::axum;
use server::textchat;
use server::voice_chat;
use shared::log::init_log;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");
    let _g = init_log("server");
    // 启动登录服务器
    tokio::spawn(async {
        if let Err(e) = axum::run_https_server().await {
            dbg!(&e);
        }
    });

    // 启动文本聊天服务器
    tokio::spawn(async {
        if let Err(e) = textchat::run().await {
            dbg!(&e);
        }
    });

    // 启动语音聊天服务器
    tokio::spawn(async {
        if let Err(e) = voice_chat::run().await {
            dbg!(&e);
        }
    });

    // 保持主线程运行
    tokio::signal::ctrl_c().await?;
    Ok(())
}
