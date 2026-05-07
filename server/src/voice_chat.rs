use bytes::BytesMut;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::{Database, DatabaseConnection};
use shared::group::GroupId;
use shared::voice_chat::S2C_VC_Msg;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_rustls::{TlsAcceptor, TlsStream};

const MAX_BUF_SIZE: usize = 100_000;

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    let online_groups = OnlineGroups::default();

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

struct VoiceGroup {
    gid: GroupId,
    sender: broadcast::Sender<S2C_VC_Msg>,
    _r: broadcast::Receiver<S2C_VC_Msg>,
}

#[derive(Default, Clone)]
struct OnlineGroups {
    hm: Arc<RwLock<HashMap<GroupId, VoiceGroup>>>,
}

async fn handle_client(
    db: DatabaseConnection,
    tls_stream: TlsStream<TcpStream>,
    online_groups: OnlineGroups,
) -> anyhow::Result<()> {
    todo!()
}

pub struct ReadHelper<T>
where
    T: AsyncReadExt,
{
    rh: T,
}
impl<T> ReadHelper<T>
where
    T: AsyncReadExt + Unpin,
{
    pub fn new(rh: T) -> Self {
        Self { rh }
    }
    pub async fn next_item(&mut self, buf: &mut BytesMut) -> Option<usize> {
        let Ok(u) = self.rh.read_u64().await else {
            return None;
        };
        let u = u as usize;

        if u > MAX_BUF_SIZE {
            return None;
        }
        //使用len: 必须是已经初始化过的长度
        if u > buf.len() {
            buf.resize(u, 0);
        }
        let Ok(ans) = self.rh.read_exact(&mut buf[..u]).await else {
            return None;
        };

        Some(ans)
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
