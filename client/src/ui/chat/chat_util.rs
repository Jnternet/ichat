use crate::entity::{account_group, groups, messages};
use sea_orm::prelude::DateTimeUtc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use shared::auth::Auth;
use shared::group::GroupId;
use std::sync::Arc;
use tokio::io::WriteHalf;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;

pub struct OneGroup {
    pub id: GroupId,
    pub name: String,
    pub last_msg: Option<String>,
    pub last_msg_time: Option<DateTimeUtc>,
}
pub struct UIGroups {
    pub groups: std::collections::VecDeque<OneGroup>,
}

pub struct OneMessage {
    pub content: String,
    pub is_mine: bool,
    pub time: DateTimeUtc,
}

pub type TlsWriteHalfInner = Arc<Mutex<WriteHalf<TlsStream<tokio::net::TcpStream>>>>;

#[derive(Clone)]
pub struct TlsWriteHalf(pub TlsWriteHalfInner);

impl std::fmt::Debug for TlsWriteHalf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TlsWriteHalf")
    }
}

pub(super) fn get_connector() -> tokio_rustls::TlsConnector {
    let root_cert_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    tokio_rustls::TlsConnector::from(Arc::new(client_config))
}

pub(super) async fn connect(
    auth: &Auth,
) -> anyhow::Result<(
    TlsWriteHalf,
    tokio::io::ReadHalf<TlsStream<tokio::net::TcpStream>>,
)> {
    use tokio::io::AsyncWriteExt;

    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    let connector = get_connector();
    let tcp = tokio::net::TcpStream::connect(&server_addr).await?;
    let stream = connector.connect(server_name.try_into()?, tcp).await?;
    let mut tls = TlsStream::from(stream);

    let auth_json = shared::serde_json::to_vec(auth)?;
    tls.write_all(&auth_json).await?;
    tls.flush().await?;

    let (rh, wh) = tokio::io::split(tls);
    Ok((TlsWriteHalf(Arc::new(Mutex::new(wh))), rh))
}

pub(super) async fn get_groups_info(
    auth: Auth,
    db: DatabaseConnection,
) -> Result<UIGroups, String> {
    let uid = auth.account_id();
    let txn = db
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let group_records = account_group::Entity::find()
        .filter(account_group::Column::AccountUuid.eq(uid))
        .all(&txn)
        .await
        .map_err(|e| format!("Failed to query account_group: {}", e))?;

    let mut groups_info: Vec<OneGroup> = Vec::new();
    for record in group_records {
        let group_id = GroupId(record.group_uuid);
        let one_group = get_one_group(&txn, group_id)
            .await
            .map_err(|e| format!("Failed to get group info: {}", e))?;
        groups_info.push(one_group);
    }

    groups_info.sort_by(
        |a, b| match (a.last_msg_time.as_ref(), b.last_msg_time.as_ref()) {
            (Some(a_time), Some(b_time)) => b_time.cmp(a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        },
    );

    txn.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(UIGroups { groups: groups_info.into() })
}

async fn get_one_group(db: &impl ConnectionTrait, id: GroupId) -> Result<OneGroup, String> {
    let group = groups::Entity::find_by_id(id.0)
        .one(db)
        .await
        .map_err(|e| format!("Failed to find group: {}", e))?
        .ok_or_else(|| format!("Group not found: {:?}", id.0))?;

    let last_message = messages::Entity::find()
        .filter(messages::Column::GroupUuid.eq(id.0))
        .order_by_desc(messages::Column::CreateAt)
        .one(db)
        .await
        .map_err(|e| format!("Failed to find messages: {}", e))?;

    let (last_msg, last_msg_time) = match last_message {
        Some(msg) => (Some(msg.content), Some(msg.create_at)),
        None => (None, None),
    };

    Ok(OneGroup { id, name: group.group_name, last_msg, last_msg_time })
}

pub(super) async fn get_group_messages(
    auth: Auth,
    db: DatabaseConnection,
    group_id: GroupId,
) -> Result<Vec<OneMessage>, String> {
    let my_id = auth.account_id();
    let msgs = messages::Entity::find()
        .filter(messages::Column::GroupUuid.eq(group_id.0))
        .order_by_asc(messages::Column::CreateAt)
        .all(&db)
        .await
        .map_err(|e| format!("Failed to query messages: {}", e))?;

    Ok(msgs
        .into_iter()
        .map(|m| OneMessage {
            content: m.content,
            is_mine: m.account_uuid == my_id,
            time: m.create_at,
        })
        .collect())
}
