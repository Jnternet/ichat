use crate::entity::{accounts, prelude::*};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse, routing::post};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use shared::register::{Register, RegisterSuccess};

// 从 axum 模块导入 AppState
use crate::axum::AppState;

#[axum::debug_handler]
pub async fn register(
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

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
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
