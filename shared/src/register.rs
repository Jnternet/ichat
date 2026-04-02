use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Register {
    pub user_name: String,
    pub account: String,
    pub password: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterSuccess;

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum RegisterError {
    #[error("this account is already existence")]
    AlreadyExist,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RegisterResponse {
    Success(RegisterSuccess),
    Fail(RegisterError),
}
