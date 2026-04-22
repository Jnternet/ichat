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

pub async fn create_group(
    client: &Client,
    url: &str,
    create_group: &CreateGroup,
) -> anyhow::Result<CreateGroupResponse> {
    let text = client
        .post(url)
        .json(create_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<CreateGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(CreateGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(CreateGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn join_group(
    client: &Client,
    url: &str,
    join_group: &JoinGroup,
) -> anyhow::Result<JoinGroupResponse> {
    let text = client
        .post(url)
        .json(join_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<JoinGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(JoinGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(JoinGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn exit_group(
    client: &Client,
    url: &str,
    exit_group: &ExitGroup,
) -> anyhow::Result<ExitGroupResponse> {
    let text = client
        .post(url)
        .json(exit_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<ExitGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(ExitGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(ExitGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn delete_group(
    client: &Client,
    url: &str,
    delete_group: &DeleteGroup,
) -> anyhow::Result<DeleteGroupResponse> {
    let text = client
        .post(url)
        .json(delete_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<DeleteGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(DeleteGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(DeleteGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn list_groups(
    client: &Client,
    url: &str,
    list_groups: &ListGroups,
) -> anyhow::Result<ListGroupsResponse> {
    let text = client
        .post(url)
        .json(list_groups)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<ListGroupsSuccess>(&text);
    if let Ok(s) = result {
        return Ok(ListGroupsResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(ListGroupsResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn get_group(
    client: &Client,
    url: &str,
    get_group: &GetGroup,
) -> anyhow::Result<GetGroupResponse> {
    let text = client
        .post(url)
        .json(get_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<GetGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(GetGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(GetGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}
