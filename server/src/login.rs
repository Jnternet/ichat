use crate::entity::accounts;
use crate::entity::auths;
use crate::entity::prelude::*;
use axum::extract::State;
use axum::{Json, response::IntoResponse};
use sea_orm::ConnectionTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use sea_orm::TransactionTrait;
use sea_orm::{ActiveModelTrait, Set};
use shared::auth::Auth;
use shared::login::*;
use tracing::{debug, info, instrument, warn};

use crate::axum::AppState;

#[instrument(skip(state, login))]
pub async fn login(
    State(state): State<AppState>,
    Json(login): Json<Login>,
) -> Result<impl IntoResponse, LoginError> {
    info!("Login attempt for account: {}", login.account);
    match _login(state, login).await {
        Ok(ir) => {
            info!("Login successful");
            Ok(ir)
        }
        Err(e) => {
            let r = e.downcast::<LoginError>();
            if r.is_err() {
                let e = r.as_ref().err().unwrap();
                error!("Login internal error: {:?}", e);
                return Err(LoginError::ServerWrong);
            }
            let err = r.unwrap();
            warn!("Login failed: {:?}", err);
            Err(err)
        }
    }
}

#[instrument(skip(state))]
async fn _login(state: AppState, login: Login) -> anyhow::Result<impl IntoResponse> {
    let db = state.db;
    let txn = db.begin().await?;

    debug!("Checking account existence: {}", login.account);
    let opt_ac = Accounts::find()
        .filter(accounts::COLUMN.account.eq(login.account.clone()))
        .one(&txn)
        .await?;

    if opt_ac.is_none() {
        warn!("Account not found: {}", login.account);
        return Err(LoginError::NotExist.into());
    }
    let ac = opt_ac.unwrap();

    if ac.password != login.password {
        warn!("Wrong password for account: {}", login.account);
        return Err(LoginError::WrongPassword.into());
    }

    let removed = remove_expired_token(&txn, &ac.uuid).await?;
    debug!("Removed {} expired tokens for account {}", removed, ac.uuid);

    let au = auths::ActiveModel {
        token: Set(uuid::Uuid::now_v7()),
        account: Set(ac.uuid),
        create_at: Set(chrono::Utc::now()),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;
    debug!("Token created for account: {}", ac.uuid);

    Ok(Json(LoginSuccess {
        auth: Auth::new(au.account, &au.token.to_string()),
    }))
}

#[instrument(skip(db))]
async fn remove_expired_token(
    db: &impl ConnectionTrait,
    account_id: &uuid::Uuid,
) -> anyhow::Result<u64> {
    let now = chrono::Utc::now();
    let token_expire_time = std::env::var("TOKEN_EXPIRE_TIME")?.parse::<i64>()?;
    let td = chrono::Duration::seconds(token_expire_time);
    let t = now - td;
    let v_a = Auths::delete_many()
        .filter(auths::COLUMN.account.eq(*account_id))
        .filter(auths::COLUMN.create_at.lt(t))
        .exec(db)
        .await?;
    debug!(
        "Removed {} expired tokens for account {}",
        v_a.rows_affected, account_id
    );
    Ok(v_a.rows_affected)
}

use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("this account does not exist")]
    NotExist,
    #[error("WrongPassword")]
    WrongPassword,
    #[error("Something wrong in server")]
    ServerWrong,
    // 不应向客户端暴露服务器错误
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
            LoginError::ServerWrong => (
                StatusCode::NOT_FOUND,
                Json(shared::login::LoginError::ServerWrong),
            )
                .into_response(),
        }
    }
}
