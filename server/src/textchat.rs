use async_broadcast::Receiver;
use rkyv::Archived;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use shared::group::GroupId;
use shared::message::S2C_Msg;
use shared::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::{TlsAcceptor, TlsStream};

const MAX_MSG_NUM: usize = 100;

pub async fn run() -> anyhow::Result<()> {
    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        eprintln!("客户端连接: {}", addr);
        let tls_stream = tls_acceptor.accept(stream).await?;
        let tls_stream = TlsStream::from(tls_stream);

        tokio::spawn(async move {
            let r = handle_client(tls_stream).await;
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

pub async fn handle_client(tls_stream: TlsStream<tokio::net::TcpStream>) -> anyhow::Result<()> {
    // let (mut rh, mut wh) = tokio::io::split(tls_stream);
    //
    // let mut buf = bytes::BytesMut::new();
    // rh.read_buf(&mut buf).await?;
    // let ar_test = rkyv::access::<Archived<Test>, rkyv::rancor::Error>(&buf)?;
    // dbg!(&ar_test);
    //
    // wh.write_all("server respond".as_bytes()).await?;
    // wh.flush().await?;
    //
    anyhow::Ok(())
}
