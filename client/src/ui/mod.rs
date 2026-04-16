use iced::{Element, Task};

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
    ChangeTo(Screen),
}

#[derive(Default)]
enum Screen {
    #[default]
    Login,
}

#[derive(Default)]
struct Screens {
    login: login::Login,
}

impl AppState {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Login(m) => self.screens.login.update(m).map(Message::Login),
            Message::ChangeTo(_s) => Task::none(),
        }
    }
    fn view(&self) -> Element<'_, Message> {
        match self.current_screen {
            Screen::Login => self.screens.login.view().map(Message::Login),
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
