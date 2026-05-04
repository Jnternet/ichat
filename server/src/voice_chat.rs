use anyhow::Context;
use anyhow::bail;
use async_broadcast::Receiver;
use futures::StreamExt;
use futures::prelude::*;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::{Database, DatabaseConnection};
use shared::account::AccountId;
use shared::group::GroupId;
use shared::voice_chat::C2S_VC_Msg;
use shared::voice_chat::S2C_VC_Msg;
use shared::voice_chat::VoiceGroupAuth;
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

//todo: 验证是否有权发送到指定的群
pub async fn handle_client(
    db: DatabaseConnection,
    tls_stream: TlsStream<tokio::net::TcpStream>,
    online_groups: OnlineGroups<S2C_VC_Msg>,
) -> anyhow::Result<()> {
    let (mut rh, wh) = tokio::io::split(tls_stream);
    let mut buf = bytes::BytesMut::with_capacity(1024);
    let u = rh
        .read_buf(&mut buf)
        .await
        .context("cannot read from client")?;

    let vga = serde_json::from_slice::<VoiceGroupAuth>(&buf[..u])
        .context("cannot get voice group auth")?;
    let auth = vga.auth;
    let gid = vga.gid;

    if crate::auth::auth(&db, &auth).await {
        bail!("no auth to voice_chat")
    }

    let r = online_groups.join(&gid).await;
    let v = vec![r];
    let sa = futures::stream::select_all(v);

    eprintln!("准备启动rh与wh");
    tokio::select! {
        r = handle_rh(rh,online_groups.clone(),AccountId(auth.account_id())) => {
            dbg!(&r);
        },
        r = handle_wh(wh,sa) => {
            dbg!(&r);
        },
    }
    eprintln!("出现错误，退出群组");
    online_groups.exit(&gid).await;

    anyhow::Ok(())
}

async fn handle_rh(
    mut read_half: ReadHalf<TlsStream<tokio::net::TcpStream>>,
    online_groups: OnlineGroups<S2C_VC_Msg>,
    sender_id: AccountId,
) -> anyhow::Result<()> {
    eprintln!("进入handle_rh");
    let mut buf = bytes::BytesMut::with_capacity(8192);
    loop {
        read_half.read_buf(&mut buf).await?;
        let msg = serde_json::from_slice::<C2S_VC_Msg>(&buf)?;
        buf.clear();
        let s2c = S2C_VC_Msg {
            sender_id,
            voice_data: msg.voice_data,
        };

        //缩短持有锁的时间
        let gs = {
            let mg = online_groups.0.lock().await;
            mg.get(&msg.target).context("没有创建在线群组")?.clone()
        };
        gs.sender.broadcast_direct(s2c).await?;
    }
}

async fn handle_wh(
    mut write_half: WriteHalf<TlsStream<tokio::net::TcpStream>>,
    mut sa: stream::SelectAll<Receiver<S2C_VC_Msg>>,
) -> anyhow::Result<()> {
    eprintln!("进入handle_wh");
    while let Some(m) = sa.next().await {
        write_half
            .write_all(serde_json::to_vec(&m)?.as_slice())
            .await?;
        write_half.flush().await?;
    }
    Ok(())
}
