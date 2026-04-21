use crate::ui::login::Action;
use iced::{Element, Task};
use sea_orm::Database;

pub mod chat;
pub mod login;

pub fn run() -> iced::Result {
    iced::application(AppState::default, AppState::update, AppState::view)
        .centered()
        .run()
}

struct AppState {
    current_screen: Screen,
}
enum Message {
    Login(login::Message),
    Chat(chat::Message),
}

enum Screen {
    Login(login::Login),
    Chat(chat::Chat),
}

impl AppState {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Login(m) => {
                if let Screen::Login(l) = &mut self.current_screen {
                    let a = l.update(m);
                    match a {
                        login::Action::None => Task::none(),
                        login::Action::Run(t) => t.map(Message::Login),
                        login::Action::ChangeToChat { auth, client, url } => {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            //准备数据库
                            let client_db_url = std::env::var("CLIENT_DATABASE").unwrap();
                            let db = rt.block_on(async move {
                                Database::connect(client_db_url).await.unwrap()
                            });
                            let c = chat::Chat::new(auth, db, client, url);
                            //切换页面
                            self.current_screen = Screen::Chat(c);
                            Task::none()
                        }
                    }
                } else {
                    Task::none()
                }
            }
            Message::Chat(m) => {
                if let Screen::Chat(c) = &mut self.current_screen {
                    let a = c.update(m);
                    match a {
                        chat::Action::None => Task::none(),
                        chat::Action::Run(t) => t.map(Message::Chat),
                        chat::Action::ChangeToLogin { client, url } => {
                            let mut l = login::Login::default();
                            l.inner.client = client;
                            l.inner.url = url;
                            self.current_screen = Screen::Login(l);
                            //切换页面
                            Task::none()
                        }
                    }
                } else {
                    Task::none()
                }
            }
        }
    }
    fn view(&self) -> Element<'_, Message> {
        todo!()
    }
}

impl Default for AppState {
    fn default() -> Self {
        let l = login::Login::default();
        Self {
            current_screen: Screen::Login(l),
        }
    }
}
