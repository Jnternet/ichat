use axum::{Router, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use sea_orm::Database;
use std::net::SocketAddr;

// 导入路由处理函数
use crate::group::{
    route_create_group, route_delete_group, route_exit_group, route_get_group, route_join_group,
    route_list_groups,
};
use crate::login::login;
use crate::register::register;
use crate::update_info::update_info;

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
}

/// 启动https服务器
pub async fn run_https_server() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;
    //准备状态
    let app_state = AppState { db };
    // 你的路由
    let app = Router::new()
        .route(r"/login", post(login))
        .route(r"/register", post(register))
        .route(r"/create_group", post(route_create_group))
        .route(r"/join_group", post(route_join_group))
        .route(r"/exit_group", post(route_exit_group))
        .route(r"/delete_group", post(route_delete_group))
        .route(r"/list_groups", post(route_list_groups))
        .route(r"/get_group", post(route_get_group))
        .route(r"/update_info", post(update_info))
        .with_state(app_state);

    // 載入證書與私鑰（PEM 格式）
    // 正式環境請使用 Let's Encrypt 或其他正規憑證
    let tls_config = RustlsConfig::from_pem_file(
        "items/cert/fullchain.pem", // 憑證（通常包含中間憑證）
        "items/cert/privkey.pem",   // 私鑰
    )
    .await?;

    let addr = std::env::var("SERVER_HTTPS_ADDR")?.parse::<SocketAddr>()?;

    // 啟動 HTTPS 伺服器
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
