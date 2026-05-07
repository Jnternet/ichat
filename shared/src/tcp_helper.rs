use bytes::BytesMut;
use tokio::io::AsyncReadExt;

const MAX_BUF_SIZE: usize = 100_000;
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
