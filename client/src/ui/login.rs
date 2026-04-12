use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use reqwest::Client;
use sea_orm::{Database, DatabaseConnection};
use shared::auth::Auth;

pub fn run() -> iced::Result {
    iced::application(Login::default, Login::update, Login::view)
        .centered()
        .run()
}

struct Login {
    inner: Inner,
    view_state: ViewState,
    username: String,
    password: String,
    confirm_password: String,
}
struct Inner {
    auth: Option<Auth>,
    db: DatabaseConnection,
    client: Client,
}
impl Default for Login {
    fn default() -> Self {
        //准备数据库
        let client_db_url = std::env::var("CLIENT_DATABASE").unwrap();
        // let db = Database::connect(client_db_url).await.unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let db = rt.block_on(async { Database::connect(client_db_url).await.unwrap() });

        let root_cert_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        let server_addr = std::env::var("SERVER_HTTPS_ADDR").unwrap();
        let server_name = std::env::var("SERVER_NAME").unwrap();

        let client = reqwest::Client::builder()
            .resolve(&server_name, server_addr.parse().unwrap())
            .tls_backend_preconfigured(client_config)
            .no_proxy()
            .build()
            .unwrap();
        let inner = Inner {
            auth: None,
            db,
            client,
        };
        Self {
            inner,
            view_state: ViewState::Login,
            username: String::new(),
            password: String::new(),
            confirm_password: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum ViewState {
    #[default]
    Login,
    Register,
}

#[derive(Debug, Clone)]
enum Message {
    UsernameChanged(String),
    PasswordChanged(String),
    ConfirmPasswordChanged(String),
    SwitchView(ViewState),
    SubmitLogin,
    SubmitRegister,
}

impl Login {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UsernameChanged(u) => self.username = u,
            Message::PasswordChanged(p) => self.password = p,
            Message::ConfirmPasswordChanged(cp) => self.confirm_password = cp,
            Message::SwitchView(view) => {
                self.view_state = view;
                self.username.clear();
                self.password.clear();
                self.confirm_password.clear();
            }
            Message::SubmitLogin => {
                println!("正在尝试登录: {}", self.username);
                // todo: 调用登录 API (例如: POST /api/v1/login)
                // 如果使用 tokio, 可以在此处返回 Task::perform(api_call, Message::LoginResponse)
            }
            Message::SubmitRegister => {
                if self.password == self.confirm_password {
                    println!("正在尝试注册: {}", self.username);
                    // todo: 调用注册 API (例如: POST /api/v1/register)
                    // 需要处理 Argon2id 哈希和 Noise 协议握手初始化
                } else {
                    println!("两次输入的密码不一致");
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text(match self.view_state {
            ViewState::Login => "欢迎回来",
            ViewState::Register => "创建新账户",
        })
        .size(30);

        let username_input = text_input("用户名", &self.username)
            .on_input(Message::UsernameChanged)
            .padding(10);

        let password_input = text_input("密码", &self.password)
            .on_input(Message::PasswordChanged)
            .secure(true)
            .padding(10);

        let mut content = column![title, username_input, password_input]
            .spacing(20)
            .max_width(300)
            .align_x(Alignment::Center);

        if self.view_state == ViewState::Register {
            content = content.push(
                text_input("确认密码", &self.confirm_password)
                    .on_input(Message::ConfirmPasswordChanged)
                    .secure(true)
                    .padding(10),
            );
        }

        let submit_button = match self.view_state {
            ViewState::Login => column![
                button("立即登录")
                    .on_press(Message::SubmitLogin)
                    .padding(10)
                    .width(Length::Fill),
                row![
                    text("还没有账号？"),
                    button("点击注册").on_press(Message::SwitchView(ViewState::Register))
                ]
                .align_y(Alignment::Center)
                .spacing(5)
            ],
            ViewState::Register => column![
                button("注册账户")
                    .on_press(Message::SubmitRegister)
                    .padding(10)
                    .width(Length::Fill),
                row![
                    text("已有账号？"),
                    button("返回登录").on_press(Message::SwitchView(ViewState::Login))
                ]
                .align_y(Alignment::Center)
                .spacing(5)
            ],
        }
        .spacing(10)
        .align_x(Alignment::Center);

        container(content.push(submit_button))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
