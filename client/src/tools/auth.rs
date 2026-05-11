use anyhow::bail;
use reqwest::Client;
use shared::login::*;
use shared::register::*;
use shared::serde_json;
use tracing::{error, info, instrument, warn};

#[instrument(skip(client))]
pub async fn login(client: &Client, url: &str, login: &Login) -> anyhow::Result<LoginResponse> {
    info!("Login request to {} for account: {}", url, login.account);
    let response = match client.post(url).json(login).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Login request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<LoginSuccess>(&text);
    if let Ok(s) = result {
        info!("Login successful for account: {}", login.account);
        return Ok(LoginResponse::Success(s));
    }
    let result = serde_json::from_str::<LoginError>(&text);
    if let Ok(e) = result {
        warn!("Login failed for account {}: {:?}", login.account, e);
        return Ok(LoginResponse::Fail(e));
    }
    error!(
        "Cannot resolve login response for account: {}",
        login.account
    );
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn register(
    client: &Client,
    url: &str,
    register: &Register,
) -> anyhow::Result<RegisterResponse> {
    info!(
        "Register request to {} for account: {}",
        url, register.account
    );
    let response = match client.post(url).json(register).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Register request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<RegisterSuccess>(&text);
    if let Ok(s) = result {
        info!("Register successful for account: {}", register.account);
        return Ok(RegisterResponse::Success(s));
    }
    let result = serde_json::from_str::<RegisterError>(&text);
    if let Ok(e) = result {
        warn!("Register failed for account {}: {:?}", register.account, e);
        return Ok(RegisterResponse::Fail(e));
    }
    error!(
        "Cannot resolve register response for account: {}",
        register.account
    );
    bail!("cannot resolve response")
}
