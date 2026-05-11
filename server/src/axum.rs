use axum::{Router, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use sea_orm::Database;
use std::net::SocketAddr;
use tracing::{error, info, instrument};

pub use shared::tracing;
pub use shared::tracing_appender;
pub use shared::tracing_subscriber;

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

#[instrument]
pub async fn run_https_server() -> anyhow::Result<()> {
    info!("Initializing HTTPS server...");

    let server_db_url = std::env::var("SERVER_DATABASE")?;
    info!("Connecting to database: {}", server_db_url);
    let db = Database::connect(server_db_url).await.map_err(|e| {
        error!("Failed to connect to database: {:?}", e);
        e
    })?;
    info!("Database connection established");

    let app_state = AppState { db };
    info!("Building router with {} routes", 9);
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

    let tls_config =
        RustlsConfig::from_pem_file("items/cert/fullchain.pem", "items/cert/privkey.pem")
            .await
            .map_err(|e| {
                error!("Failed to load TLS certificates: {:?}", e);
                e
            })?;
    info!("TLS configuration loaded");

    let addr = std::env::var("SERVER_HTTPS_ADDR")?.parse::<SocketAddr>()?;
    info!("Starting HTTPS server on {}", addr);

    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .map_err(|e| {
            error!("HTTPS server error: {:?}", e);
            e
        })?;

    info!("HTTPS server stopped");
    Ok(())
}
