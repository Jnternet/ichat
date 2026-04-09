use crate::entity::{accounts, prelude::*};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, response::IntoResponse, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};
use shared::register::{Register, RegisterSuccess};
use std::net::SocketAddr;

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    //准备状态
    let app_state = AppState { db };
    // 你的路由
    let app = Router::new()
        .route(r"/register", post(register))
        .with_state(app_state);

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
async fn register(
    State(state): State<AppState>,
    Json(register): Json<Register>,
) -> Result<impl IntoResponse, RegisterError> {
    if let Err(e) = _register(state, register).await {
        dbg!(&e);
        match e.downcast::<RegisterError>() {
            Ok(o) => {
                return Err(o);
            }
            Err(e) => {
                dbg!(&e);
            }
        };
    }
    Ok(Json(RegisterSuccess))
}
async fn _register(state: AppState, register: Register) -> anyhow::Result<impl IntoResponse> {
    let db = state.db;
    let txn = db.begin().await?;
    let opt_account = Accounts::find()
        .filter(accounts::COLUMN.account.eq(register.account.clone()))
        .one(&txn)
        .await?;
    if opt_account.is_some() {
        return Err(RegisterError::AlreadyExist.into());
    }

    let _m = accounts::ActiveModel {
        uuid: Set(uuid::Uuid::now_v7()),
        user_name: Set(register.user_name),
        account: Set(register.account),
        password: Set(register.password),
        create_at: Set(chrono::Utc::now()),
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;

    Ok(Json(RegisterSuccess))
}
#[derive(Debug, Clone)]
struct AppState {
    db: DatabaseConnection,
}

#[derive(Debug, thiserror::Error)]
enum RegisterError {
    #[error("this account is already existence")]
    AlreadyExist,
}
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
