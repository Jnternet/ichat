use migration::MigratorTrait;
use rustls::crypto::aws_lc_rs;
use sea_orm::Database;
use std::thread::park;

mod auth;
mod entity;
mod login;
mod register;
mod textchat;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    //为rustls选择provider
    aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc-rs crypto provider");
    //根据migration迁移数据库架构
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;
    migration::Migrator::up(&db, None).await?;
    //只用于迁移架构
    drop(db);

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
    Ok(())
}
