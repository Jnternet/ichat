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
    screens: Screens,
}
enum Message {
    Login(login::Message),
    Chat(chat::Message),
}

#[derive(Default)]
enum Screen {
    #[default]
    Login,
    Chat,
}

#[derive(Default)]
struct Screens {
    login: login::Login,
    chat: chat::Chat,
}

impl AppState {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Login(m) => {
                let a = self.screens.login.update(m);
                match a {
                    Action::None => Task::none(),
                    Action::Run(t) => t.map(Message::Login),
                    Action::ChangeToChat => {
                        self.current_screen = Screen::Chat;
                        // if let Some(i) = &self.screens.chat.inner {
                        //     return Task::none();
                        // }
                        let auth = self.screens.login.inner.auth.clone().unwrap();
                        let client = self.screens.login.inner.client.clone();
                        let url = self.screens.login.inner.url.clone();

                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let db = rt.block_on(async move {
                            //准备数据库
                            let client_db_url = std::env::var("CLIENT_DATABASE").unwrap();
                            Database::connect(client_db_url).await.unwrap()
                        });
                        //新建立的chat页面
                        self.screens.chat = chat::Chat::new(auth, db, client, url);
                        Task::none()
                    }
                }
            }
            Message::Chat(m) => Task::none(),
        }
    }
    fn view(&self) -> Element<'_, Message> {
        match self.current_screen {
            Screen::Login => self.screens.login.view().map(Message::Login),
            Screen::Chat => self.screens.chat.view().map(Message::Chat),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Login,
            screens: Screens::default(),
        }
    }
}
