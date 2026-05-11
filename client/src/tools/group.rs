use anyhow::bail;
use reqwest::Client;
use shared::group::CreateGroup;
use shared::group::CreateGroupResponse;
use shared::group::CreateGroupSuccess;
use shared::group::DeleteGroup;
use shared::group::DeleteGroupResponse;
use shared::group::DeleteGroupSuccess;
use shared::group::ExitGroup;
use shared::group::ExitGroupResponse;
use shared::group::ExitGroupSuccess;
use shared::group::GetGroup;
use shared::group::GetGroupResponse;
use shared::group::GetGroupSuccess;
use shared::group::GroupError;
use shared::group::JoinGroup;
use shared::group::JoinGroupResponse;
use shared::group::JoinGroupSuccess;
use shared::group::ListGroups;
use shared::group::ListGroupsResponse;
use shared::group::ListGroupsSuccess;
use shared::serde_json;
use tracing::{error, info, instrument, warn};

#[instrument(skip(client))]
pub async fn create_group(
    client: &Client,
    url: &str,
    create_group: &CreateGroup,
) -> anyhow::Result<CreateGroupResponse> {
    info!("Create group request to {}: {}", url, create_group.name);
    let response = match client.post(url).json(create_group).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Create group request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<CreateGroupSuccess>(&text);
    if let Ok(s) = result {
        info!("Create group successful: {}", s.group_id.0);
        return Ok(CreateGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("Create group failed: {:?}", e);
        return Ok(CreateGroupResponse::Fail(e));
    }
    error!("Cannot resolve create group response");
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn join_group(
    client: &Client,
    url: &str,
    join_group: &JoinGroup,
) -> anyhow::Result<JoinGroupResponse> {
    info!(
        "Join group request: account={}, group={}",
        join_group.auth.account_id(),
        join_group.group_id.0
    );
    let response = match client.post(url).json(join_group).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Join group request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<JoinGroupSuccess>(&text);
    if let Ok(s) = result {
        info!(
            "Join group successful: account={}, group={}",
            s.uid.0, s.gid.0
        );
        return Ok(JoinGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("Join group failed: {:?}", e);
        return Ok(JoinGroupResponse::Fail(e));
    }
    error!("Cannot resolve join group response");
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn exit_group(
    client: &Client,
    url: &str,
    exit_group: &ExitGroup,
) -> anyhow::Result<ExitGroupResponse> {
    info!(
        "Exit group request: account={}, group={}",
        exit_group.auth.account_id(),
        exit_group.group_id.0
    );
    let response = match client.post(url).json(exit_group).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Exit group request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<ExitGroupSuccess>(&text);
    if let Ok(s) = result {
        info!("Exit group successful");
        return Ok(ExitGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("Exit group failed: {:?}", e);
        return Ok(ExitGroupResponse::Fail(e));
    }
    error!("Cannot resolve exit group response");
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn delete_group(
    client: &Client,
    url: &str,
    delete_group: &DeleteGroup,
) -> anyhow::Result<DeleteGroupResponse> {
    info!(
        "Delete group request: account={}, group={}",
        delete_group.auth.account_id(),
        delete_group.group_id.0
    );
    let response = match client.post(url).json(delete_group).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Delete group request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<DeleteGroupSuccess>(&text);
    if let Ok(s) = result {
        info!("Delete group successful");
        return Ok(DeleteGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("Delete group failed: {:?}", e);
        return Ok(DeleteGroupResponse::Fail(e));
    }
    error!("Cannot resolve delete group response");
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn list_groups(
    client: &Client,
    url: &str,
    list_groups: &ListGroups,
) -> anyhow::Result<ListGroupsResponse> {
    info!(
        "List groups request for account: {}",
        list_groups.auth.account_id()
    );
    let response = match client.post(url).json(list_groups).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("List groups request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<ListGroupsSuccess>(&text);
    if let Ok(s) = result {
        info!("List groups successful, found {} groups", s.groups.len());
        return Ok(ListGroupsResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("List groups failed: {:?}", e);
        return Ok(ListGroupsResponse::Fail(e));
    }
    error!("Cannot resolve list groups response");
    bail!("cannot resolve response")
}

#[instrument(skip(client))]
pub async fn get_group(
    client: &Client,
    url: &str,
    get_group: &GetGroup,
) -> anyhow::Result<GetGroupResponse> {
    info!(
        "Get group request: account={}, group={}",
        get_group.auth.account_id(),
        get_group.group_id.0
    );
    let response = match client.post(url).json(get_group).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Get group request failed: {:?}", e);
            return Err(e.into());
        }
    };

    let text = response.text().await?;
    let result = serde_json::from_str::<GetGroupSuccess>(&text);
    if let Ok(s) = result {
        info!("Get group successful: {} ({})", s.group.id.0, s.group.name);
        return Ok(GetGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        warn!("Get group failed: {:?}", e);
        return Ok(GetGroupResponse::Fail(e));
    }
    error!("Cannot resolve get group response");
    bail!("cannot resolve response")
}
