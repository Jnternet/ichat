use iced::{Element, Subscription, Task, Theme, color};
use sea_orm::Database;

pub mod chat;
pub mod login;

pub fn run() -> iced::Result {
    let custom_theme = custom_theme();
    iced::application(AppState::default, AppState::update, AppState::view)
        .subscription(AppState::subscription)
        .theme(custom_theme)
        .centered()
        .run()
}

fn custom_theme() -> Theme {
    use iced::theme::Palette;
    let custom_palette = Palette {
        background: color!(0xffffff),
        text: color!(0x000000),
        primary: color!(0x2e7d32), // 绿色代替蓝色
        success: color!(0x12664f),
        warning: color!(0xb77e33),
        danger: color!(0xc3423f),
    };
    Theme::custom("NoBlueTheme", custom_palette)
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
                            let (c, task) = chat::Chat::new(auth, db, client, url);
                            //切换页面
                            self.current_screen = Screen::Chat(c);
                            task.map(Message::Chat)
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
        match &self.current_screen {
            Screen::Login(l) => l.view().map(Message::Login),
            Screen::Chat(c) => c.view().map(Message::Chat),
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        match &self.current_screen {
            Screen::Chat(c) => c.subscription().map(Message::Chat),
            _ => Subscription::none(),
        }
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
