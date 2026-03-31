use axum::{
    Json, Router,
    response::IntoResponse,
    routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
use shared::login::*;
use std::net::SocketAddr;

pub async fn run() -> anyhow::Result<()> {
    // 你的路由
    let app = Router::new()
        .route(r"/h", get(|| async { "Hello, TLS! 🔒" }))
        .route(r"/login", post(login));

    // 載入證書與私鑰（PEM 格式）
    // 正式環境請使用 Let's Encrypt 或其他正規憑證
    let tls_config = RustlsConfig::from_pem_file(
        "items/cert/fullchain.pem", // 憑證（通常包含中間憑證）
        "items/cert/privkey.pem",   // 私鑰
    )
    .await?;

    let addr = std::env::var("SERVER_SOCK_ADDR")?.parse::<SocketAddr>()?;

    // 啟動 HTTPS 伺服器
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[axum::debug_handler]
async fn login(Json(login): Json<Login>) -> Result<impl IntoResponse, LoginError> {
    let correct = Login {
        user_name: "123".to_string(),
        password: "123".to_string(),
    };
    if login.user_name == correct.user_name && login.password == correct.password {
        return Ok(Json(LoginSuccess {
            auth: "123".to_string(),
        }));
    }
    Err(LoginError::NotExist)
}

#[derive(Debug, thiserror::Error)]
enum LoginError {
    #[error("this account does not exist")]
    NotExist,
    #[error("WrongPassword")]
    WrongPassword,
    // #[error("Internal Error: {0}")]
    // Internal(#[from] anyhow::Error),
}
use axum::http::StatusCode;
impl IntoResponse for LoginError {
    fn into_response(self) -> axum::response::Response {
        match self {
            LoginError::NotExist => (
                StatusCode::NOT_FOUND,
                Json(shared::login::LoginError::NotExist),
            )
                .into_response(),

            LoginError::WrongPassword => (
                StatusCode::NOT_FOUND,
                Json(shared::login::LoginError::WrongPassword),
            )
                .into_response(),
        }
    }
}
