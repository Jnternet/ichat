use crate::entity::accounts;
use crate::entity::auths;
use crate::entity::prelude::*;
use axum::extract::State;
use axum::{Json, response::IntoResponse, routing::post};
use sea_orm::ConnectionTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use sea_orm::{ActiveModelTrait, Set};
use sea_orm::{TransactionTrait};
use shared::auth::Auth;
use shared::login::*;

// 从 axum 模块导入 AppState
use crate::axum::AppState;

#[axum::debug_handler]
pub async fn login(
    State(state): State<AppState>,
    Json(login): Json<Login>,
) -> Result<impl IntoResponse, LoginError> {
    match _login(state, login).await {
        Ok(ir) => Ok(ir),
        Err(e) => {
            let r = e.downcast::<LoginError>();
            if r.is_err() {
                let e = r.as_ref().err().unwrap();
                dbg!(&e);
                return Err(LoginError::ServerWrong);
            }
            Err(r.unwrap())
        }
    }
}

async fn _login(state: AppState, login: Login) -> anyhow::Result<impl IntoResponse> {
    let db = state.db;
    let txn = db.begin().await?;

    //查看登录请求是否合规
    let opt_ac = Accounts::find()
        .filter(accounts::COLUMN.account.eq(login.account))
        .one(&txn)
        .await?;
    //是否存在
    if opt_ac.is_none() {
        return Err(LoginError::NotExist.into());
    }
    let ac = opt_ac.unwrap();
    //密码是否正确
    if ac.password != login.password {
        return Err(LoginError::WrongPassword.into());
    }
    //删除过期token
    let _dr = remove_expired_token(&txn, &ac.uuid).await?;

    //此时必然账号存在且密码正确
    //创建令牌
    let au = auths::ActiveModel {
        token: Set(uuid::Uuid::now_v7()),
        account: Set(ac.uuid),
        create_at: Set(chrono::Utc::now()),
    }
    .insert(&txn)
    .await?;

    //事务提交
    txn.commit().await?;

    Ok(Json(LoginSuccess {
        auth: Auth::new(au.account, &au.token.to_string()),
    }))
}
async fn remove_expired_token(
    db: &impl ConnectionTrait,
    account_id: &uuid::Uuid,
) -> anyhow::Result<u64> {
    let now = chrono::Utc::now();
    let token_expire_time = std::env::var("TOKEN_EXPIRE_TIME")?.parse::<i64>()?;
    let td = chrono::Duration::seconds(token_expire_time);
    //这是最后的未超期时间
    let t = now - td;
    let v_a = Auths::delete_many()
        .filter(auths::COLUMN.account.eq(*account_id))
        .filter(auths::COLUMN.create_at.lt(t))
        .exec(db)
        .await?;
    eprintln!("删除过期token共:{}条,uuid={account_id}", &v_a.rows_affected);
    Ok(v_a.rows_affected)
}

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
