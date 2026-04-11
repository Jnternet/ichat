use anyhow::Result;
use rustls::crypto::aws_lc_rs;
use server::axum;
use server::textchat;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");
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

    // 保持主线程运行
    tokio::signal::ctrl_c().await?;
    Ok(())
}
