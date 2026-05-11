use crate::entity::account_group;
use crate::entity::prelude::*;
use crate::message::save_msg;
use anyhow::Context;
use async_broadcast::Receiver;
use futures::StreamExt;
use futures::prelude::*;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::QueryFilter;
use sea_orm::{Database, DatabaseConnection, EntityTrait};
use shared::account::UserInfo;
use shared::auth::Auth;
use shared::group::GroupId;
use shared::message::{C2S_Msg, S2C_Msg};
use shared::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_rustls::{TlsAcceptor, TlsStream};
use tracing::{debug, error, info, instrument, warn};

const MAX_MSG_NUM: usize = 100;

#[instrument]
pub async fn run() -> anyhow::Result<()> {
    info!("Initializing text chat server...");

    let server_db_url = std::env::var("SERVER_DATABASE")?;
    info!("Connecting to database: {}", server_db_url);
    let db = Database::connect(server_db_url).await.map_err(|e| {
        error!("Failed to connect to database: {:?}", e);
        e
    })?;
    info!("Database connection established");

    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    info!("Binding to address: {}", server_addr);
    let listener = TcpListener::bind(server_addr.clone()).await.map_err(|e| {
        error!("Failed to bind to address {}: {:?}", server_addr, e);
        e
    })?;

    let tls_acceptor = get_acceptor().await.map_err(|e| {
        error!("Failed to create TLS acceptor: {:?}", e);
        e
    })?;
    info!("TLS acceptor created");

    let online_groups = OnlineGroups::new();
    info!("Text chat server ready, waiting for connections...");

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Client connected: {}", addr);

        let tls_stream = match tls_acceptor.accept(stream).await {
            Ok(s) => TlsStream::from(s),
            Err(e) => {
                warn!("TLS handshake failed for {}: {:?}", addr, e);
                continue;
            }
        };

        let db_ = db.clone();
        let online_groups_ = online_groups.clone();
        tokio::spawn(async move {
            let r = handle_client(db_, tls_stream, online_groups_).await;
            if r.is_err() {
                error!("Client handler error: {:?}", r);
            }
        });
    }
}
#[derive(Debug, Clone)]
pub struct OnlineGroups<T>(Arc<Mutex<HashMap<GroupId, GroupSender<T>>>>);
#[derive(Debug, Clone)]
struct GroupSender<T> {
    counter: usize,
    sender: async_broadcast::Sender<T>,
    //This prevents the channel from being closed after it is created.
    _recv: async_broadcast::Receiver<T>,
}
impl<T> GroupSender<T> {
    fn new(sender: async_broadcast::Sender<T>, _recv: async_broadcast::Receiver<T>) -> Self {
        GroupSender {
            counter: 0,
            sender,
            _recv,
        }
    }
    fn join(&mut self) {
        self.counter += 1;
    }
    fn exit(&mut self) {
        if self.counter != 0 {
            self.counter -= 1;
        }
    }
}
impl<T> OnlineGroups<T> {
    fn new() -> Self {
        OnlineGroups(Arc::new(Mutex::new(HashMap::new())))
    }
    async fn join(&self, group: &GroupId) -> Receiver<T> {
        let mut mg = self.0.lock().await;
        let gs = mg.entry(*group).or_insert_with(|| {
            let (sender, recv) = async_broadcast::broadcast::<T>(MAX_MSG_NUM);
            GroupSender::new(sender, recv)
        });
        gs.join();
        gs.sender.new_receiver()
    }
    async fn exit(&self, group: &GroupId) {
        // 1. 仅加一次锁 ✅ 杜绝死锁
        let mut mg = self.0.lock().await;

        // 2. 匹配群组状态：存在则处理，不存在直接忽略
        match mg.entry(*group) {
            // 群组存在：执行退出逻辑
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let group_sender = entry.get_mut();
                // 执行退出操作
                group_sender.exit();

                // 3. 关键：如果群组无任何接收器，删除群组（释放内存）
                if group_sender.counter == 0 {
                    entry.remove();
                }
            }
            // 群组不存在：直接返回，不做任何操作 ✅ 无panic
            std::collections::hash_map::Entry::Vacant(_) => {}
        }
    }
}

pub async fn get_acceptor() -> anyhow::Result<TlsAcceptor> {
    let cert_path = std::env::var("CERT_PATH")?;
    let key_path = std::env::var("KEY_PATH")?;
    let certs = CertificateDer::pem_file_iter(cert_path)?
        .map(|cert| cert.unwrap())
        .collect::<Vec<_>>();
    let key = PrivateKeyDer::from_pem_file(key_path)?;

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    anyhow::Ok(TlsAcceptor::from(Arc::new(server_config)))
}

#[instrument(skip(db, online_groups))]
pub async fn handle_client(
    db: DatabaseConnection,
    tls_stream: TlsStream<tokio::net::TcpStream>,
    online_groups: OnlineGroups<S2C_Msg>,
) -> anyhow::Result<()> {
    let (mut rh, wh) = tokio::io::split(tls_stream);
    let mut buf = bytes::BytesMut::with_capacity(1024);
    let u = rh
        .read_buf(&mut buf)
        .await
        .context("cannot read from client")?;
    let auth = serde_json::from_slice::<Auth>(&buf[..u]).context("cannot get auth")?;
    debug!("Received auth from account: {}", auth.account_id());

    let v_ag: Vec<_> = AccountGroup::find()
        .filter(account_group::COLUMN.account_uuid.eq(auth.account_id()))
        .all(&db)
        .await?
        .iter()
        .map(|m| GroupId(m.group_uuid))
        .collect();
    debug!("User {} joined {} groups", auth.account_id(), v_ag.len());

    let mut v = Vec::new();
    for gid in &v_ag {
        v.push(online_groups.join(gid).await);
    }
    let sa = futures::stream::select_all(v);
    info!(
        "Starting read/write handlers for account: {}",
        auth.account_id()
    );

    let auth_ = auth.clone();
    tokio::select! {
        r = handle_rh(db, rh, online_groups.clone(), auth_) => {
            if r.is_err() {
                warn!("Read handler error: {:?}", r);
            }
        },
        r = handle_wh(wh, sa) => {
            if r.is_err() {
                warn!("Write handler error: {:?}", r);
            }
        },
    }

    info!(
        "Disconnecting, exiting all groups for account: {}",
        auth.account_id()
    );
    for gid in &v_ag {
        online_groups.exit(gid).await
    }

    anyhow::Ok(())
}

#[instrument(skip(db, online_groups))]
async fn handle_rh(
    db: DatabaseConnection,
    mut read_half: ReadHalf<TlsStream<tokio::net::TcpStream>>,
    online_groups: OnlineGroups<S2C_Msg>,
    _auth: Auth,
) -> anyhow::Result<()> {
    info!("Starting read handler for account: {}", _auth.account_id());
    loop {
        let mut buf = bytes::BytesMut::with_capacity(1024);
        read_half.read_buf(&mut buf).await?;
        let msg = serde_json::from_slice::<C2S_Msg>(&buf)?;
        debug!(
            "Received message from {} to group {}",
            _auth.account_id(),
            msg.target().0
        );

        let m = save_msg(&db, msg.clone()).await?;
        let sender_id = msg.auth().account_id();
        let sender_name = Accounts::find_by_id(sender_id)
            .one(&db)
            .await?
            .unwrap()
            .user_name;
        let s2c = S2C_Msg::new(
            m.uuid,
            UserInfo::new(sender_id, &sender_name),
            msg.msg().to_owned(),
            *msg.target(),
            msg.time(),
        );

        let gs = {
            let mg = online_groups.0.lock().await;
            mg.get(msg.target()).context("没有创建在线群组")?.clone()
        };
        gs.sender.broadcast_direct(s2c).await?;
        debug!("Broadcast message to group {}", msg.target().0);
    }
}

#[instrument]
async fn handle_wh(
    mut write_half: WriteHalf<TlsStream<tokio::net::TcpStream>>,
    mut sa: stream::SelectAll<Receiver<S2C_Msg>>,
) -> anyhow::Result<()> {
    info!("Starting write handler");
    while let Some(m) = sa.next().await {
        write_half
            .write_all(serde_json::to_vec(&m)?.as_slice())
            .await?;
        write_half.flush().await?;
        debug!("Sent message to client");
    }
    Ok(())
}
