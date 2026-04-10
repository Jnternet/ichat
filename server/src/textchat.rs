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
use shared::account::OtherUser;
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

const MAX_MSG_NUM: usize = 100;

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    let online_groups = OnlineGroups::new();

    loop {
        let (stream, addr) = listener.accept().await?;
        eprintln!("客户端连接: {}", addr);
        let tls_stream = tls_acceptor.accept(stream).await?;
        let tls_stream = TlsStream::from(tls_stream);

        let db_ = db.clone();
        let online_groups_ = online_groups.clone();
        tokio::spawn(async move {
            let r = handle_client(db_, tls_stream, online_groups_).await;
            if r.is_err() {
                dbg!(&r);
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
}
impl<T> GroupSender<T> {
    fn new(sender: async_broadcast::Sender<T>) -> Self {
        GroupSender { counter: 0, sender }
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
            let (sender, _) = async_broadcast::broadcast::<T>(MAX_MSG_NUM);
            GroupSender::new(sender)
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

pub async fn handle_client(
    db: DatabaseConnection,
    tls_stream: TlsStream<tokio::net::TcpStream>,
    online_groups: OnlineGroups<S2C_Msg>,
) -> anyhow::Result<()> {
    let (mut rh, wh) = tokio::io::split(tls_stream);
    let mut buf = Vec::with_capacity(1024);
    let u = rh
        .read_buf(&mut buf)
        .await
        .context("cannot read from client")?;
    dbg!(&u);
    dbg!(&String::from_utf8_lossy(&buf[..u]));
    let auth = serde_json::from_slice::<Auth>(&buf[..u]).context("cannot get auth")?;
    buf.clear();
    let v_ag: Vec<_> = AccountGroup::find()
        .filter(account_group::COLUMN.account_uuid.eq(auth.account_id()))
        .all(&db)
        .await?
        .iter()
        .map(|m| GroupId(m.group_uuid))
        .collect();
    let mut v = Vec::new();
    for gid in &v_ag {
        //理应都有,不应凋亡
        v.push(online_groups.join(gid).await);
    }
    let sa = futures::stream::select_all(v);
    eprintln!("准备启动rh与wh");
    tokio::select! {
        r = handle_rh(db,rh,online_groups.clone(),auth) => {
            dbg!(&r);
        },
        r = handle_wh(wh,sa) => {
            dbg!(&r);
        },
    }
    eprintln!("出现错误，退出所有群组");
    for gid in &v_ag {
        online_groups.exit(gid).await
    }

    anyhow::Ok(())
}

//todo: 验证是否有权发送到指定的群
async fn handle_rh(
    db: DatabaseConnection,
    mut read_half: ReadHalf<TlsStream<tokio::net::TcpStream>>,
    online_groups: OnlineGroups<S2C_Msg>,
    _auth: Auth,
) -> anyhow::Result<()> {
    eprintln!("进入handle_rh");
    loop {
        let mut buf = Vec::with_capacity(1024);
        read_half.read_buf(&mut buf).await?;
        let msg = serde_json::from_slice::<C2S_Msg>(&buf)?;
        buf.clear();
        //保存到数据库
        save_msg(&db, msg.clone()).await?;
        let sender_id = msg.auth().account_id();
        let sender_name = Accounts::find_by_id(sender_id)
            .one(&db)
            .await?
            .unwrap()
            .user_name;
        let s2c = S2C_Msg::new(OtherUser::new(sender_name), msg.msg().to_owned());
        //缩短持有锁的时间
        let gs = {
            let mg = online_groups.0.lock().await;
            mg.get(msg.target()).context("没有创建在线群组")?.clone()
        };
        gs.sender.broadcast(s2c).await?;
    }
}

async fn handle_wh(
    mut write_half: WriteHalf<TlsStream<tokio::net::TcpStream>>,
    mut sa: stream::SelectAll<Receiver<S2C_Msg>>,
) -> anyhow::Result<()> {
    eprintln!("进入handle_wh");
    while let Some(m) = sa.next().await {
        write_half
            .write_all(serde_json::to_vec(&m)?.as_slice())
            .await?;
    }
    Ok(())
}
