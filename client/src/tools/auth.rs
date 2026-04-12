use anyhow::bail;
use reqwest::Client;
use shared::login::*;
use shared::register::*;
use shared::serde_json;

pub async fn login(client: &Client, url: &str, login: &Login) -> anyhow::Result<LoginResponse> {
    let text = client.post(url).json(login).send().await?.text().await?;
    let result = serde_json::from_str::<LoginSuccess>(&text);
    if let Ok(s) = result {
        return Ok(LoginResponse::Success(s));
    }
    let result = serde_json::from_str::<LoginError>(&text);
    if let Ok(e) = result {
        return Ok(LoginResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn register(
    client: &Client,
    url: &str,
    register: &Register,
) -> anyhow::Result<RegisterResponse> {
    let text = client.post(url).json(register).send().await?.text().await?;
    let result = serde_json::from_str::<RegisterSuccess>(&text);
    if let Ok(s) = result {
        return Ok(RegisterResponse::Success(s));
    }
    let result = serde_json::from_str::<RegisterError>(&text);
    if let Ok(e) = result {
        return Ok(RegisterResponse::Fail(e));
    }
    bail!("cannot resolve response")
}