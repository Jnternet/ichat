use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Register {
    pub user_name: String,
    pub account: String,
    pub password: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegisterSuccess;

#[derive(Debug, thiserror::Error, Serialize, Deserialize, Clone)]
pub enum RegisterError {
    #[error("this account is already existence")]
    AlreadyExist,
    #[error("Server error")]
    ServerWrong,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RegisterResponse {
    Success(RegisterSuccess),
    Fail(RegisterError),
}
