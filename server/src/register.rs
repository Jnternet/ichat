use axum::{Json, Router, response::IntoResponse, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use sea_orm::Database;
use shared::register::{Register, RegisterSuccess};
use std::net::SocketAddr;

pub async fn run() -> anyhow::Result<()> {
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;
    // 你的路由
    let app = Router::new().route(r"/register", post(register));

    // 載入證書與私鑰（PEM 格式）
    // 正式環境請使用 Let's Encrypt 或其他正規憑證
    let tls_config = RustlsConfig::from_pem_file(
        "items/cert/fullchain.pem", // 憑證（通常包含中間憑證）
        "items/cert/privkey.pem",   // 私鑰
    )
    .await?;

    let addr = std::env::var("SERVER_REGISTER_ADDR")?.parse::<SocketAddr>()?;

    // 啟動 HTTPS 伺服器
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[axum::debug_handler]
async fn register(Json(register): Json<Register>) -> Result<impl IntoResponse, RegisterError> {
    let correct = Register {
        user_name: "123".to_string(),
        account: "123".to_string(),
        password: "123".to_string(),
    };
    if register.account == correct.account {
        return Err(RegisterError::AlreadyExist);
    }

    Ok(Json(RegisterSuccess))
}

#[derive(Debug, thiserror::Error)]
enum RegisterError {
    #[error("this account is already existence")]
    AlreadyExist,
}
use axum::http::StatusCode;
impl IntoResponse for RegisterError {
    fn into_response(self) -> axum::response::Response {
        match self {
            RegisterError::AlreadyExist => (
                StatusCode::BAD_REQUEST,
                Json(shared::register::RegisterError::AlreadyExist),
            )
                .into_response(),
        }
    }
}
