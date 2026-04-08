use crate::entity::accounts;
use crate::entity::prelude::*;
use crate::message::save_msg;
use anyhow::Context;
use async_broadcast::Receiver;
use rkyv::Archived;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::{Database, DatabaseConnection, EntityTrait};
use shared::account::OtherUser;
use shared::group::GroupId;
use shared::message::{C2S_Msg, Msg, S2C_Msg};
use shared::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpListener;
use tokio_rustls::{TlsAcceptor, TlsStream};

const MAX_MSG_NUM: usize = 100;

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        eprintln!("客户端连接: {}", addr);
        let tls_stream = tls_acceptor.accept(stream).await?;
        let tls_stream = TlsStream::from(tls_stream);

        let db_ = db.clone();
        tokio::spawn(async move {
            let r = handle_client(db_, tls_stream).await;
            if r.is_err() {
                dbg!(&r);
            }
        });
    }
}
struct OnlineGroups<T>(HashMap<GroupId, GroupSender<T>>);
#[derive(Debug)]
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
        OnlineGroups(HashMap::new())
    }
    fn join(&mut self, group: &GroupId) -> Receiver<T> {
        let option = self.0.get(group);
        if option.is_none() {
            let (sender, _) = async_broadcast::broadcast::<T>(MAX_MSG_NUM);
            self.0.insert(*group, GroupSender::new(sender));
        }
        //此时必有该群组,不会崩溃
        let gs = self.0.get_mut(group).unwrap();
        gs.join();
        gs.sender.new_receiver()
    }
    fn exit(&mut self, group: &GroupId) {
        let option = self.0.get_mut(group);
        if option.is_none() {
            return;
        }
        let gs = option.unwrap();
        gs.exit();
        if gs.counter == 0 {
            self.0.remove(group);
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
) -> anyhow::Result<()> {
    let (rh, wh) = tokio::io::split(tls_stream);
    anyhow::Ok(())
}

async fn handle_rh(
    db: DatabaseConnection,
    mut read_half: ReadHalf<TlsStream<tokio::net::TcpStream>>,
    online_groups: &mut OnlineGroups<S2C_Msg>,
) -> anyhow::Result<()> {
    loop {
        let mut buf = vec![0u8; 1024];
        read_half.read_buf(&mut buf).await?;
        let msg = serde_json::from_slice::<C2S_Msg>(&buf)?;
        //保存到数据库
        save_msg(&db, msg.clone()).await?;
        let sender_id = msg.auth().account_id();
        let sender_name = Accounts::find_by_id(sender_id)
            .one(&db)
            .await?
            .unwrap()
            .user_name;
        let s2c = S2C_Msg::new(OtherUser::new(sender_name), msg.msg().to_owned());
        let gs = online_groups
            .0
            .get_mut(msg.target())
            .context("没有创建在线群组")?;
        gs.sender.broadcast(s2c).await?;
    }
}

async fn handle_wh(read_half: WriteHalf<TlsStream<tokio::net::TcpStream>>) -> anyhow::Result<()> {
    todo!()
}
