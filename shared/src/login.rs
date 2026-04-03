use serde::{Deserialize, Serialize};

use crate::auth::Auth;

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    pub account: String,
    pub password: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub auth: Auth,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum LoginError {
    #[error("this account does not exist")]
    NotExist,
    #[error("WrongPassword")]
    WrongPassword,
    #[error("Something wrong in server")]
    ServerWrong,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LoginResponse {
    Success(LoginSuccess),
    Fail(LoginError),
}
