use iced::widget::{button, container, text};
use iced::{Alignment, Element, Task};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;

mod chat_util;

#[derive(Default)]
pub struct Chat {
    pub inner: Option<Inner>,
}

#[derive(Clone)]
pub struct Inner {
    auth: Auth,
    db: DatabaseConnection,
    client: Client,
    url: String,
}

pub enum Message {
    Redraw,
    Exit,
}

pub enum Action {
    None,
    Run(Task<Message>),
    ChangeToLogin { client: Client, url: String },
}
impl Chat {
    pub fn new(auth: Auth, db: DatabaseConnection, client: Client, url: String) -> Self {
        Self {
            inner: Some(Inner {
                auth,
                db,
                client,
                url,
            }),
        }
    }
    pub fn update(&mut self, message: Message) -> Action {
        todo!()
    }
    pub fn view(&self) -> Element<'_, Message> {
        let mut content = iced::widget::column![]
            .spacing(20)
            .max_width(300)
            .align_x(Alignment::Center);
        let s = format!("chat page,my auth={:?}", self.inner.clone().unwrap().auth);
        let t = text(s);
        content = content.push(t);
        container(content).into()
    }
}
