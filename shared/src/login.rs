use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub auth: String,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum LoginError {
    #[error("this account does not exist")]
    NotExist,
    #[error("WrongPassword")]
    WrongPassword,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LoginResponse {
    Success(LoginSuccess),
    Fail(LoginError),
}
