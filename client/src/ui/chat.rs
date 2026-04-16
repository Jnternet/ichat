use iced::{Element, Task};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;

pub struct Chat {
    inner: Inner,
}

struct Inner {
    auth: Auth,
    db: DatabaseConnection,
    client: Client,
    url: String,
}

pub enum Message {
    Redraw,
    Exit,
}
impl Chat {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        todo!()
    }
    pub fn view(&self) -> Element<'_, Message> {
        todo!()
    }
}
impl Default for Chat {
    fn default() -> Self {
        todo!()
    }
}
