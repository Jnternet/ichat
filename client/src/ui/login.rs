use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use reqwest::Client;
use sha2::Digest;
use shared::auth::Auth;
use shared::login::{Login as SharedLogin, LoginResponse};
use shared::register::{Register, RegisterResponse};

pub struct Login {
    pub inner: Inner,
    view_state: ViewState,
    account: String,
    username: String,
    password: String,
    confirm_password: String,
    error: Option<String>,
}
pub struct Inner {
    pub auth: Option<Auth>,
    pub client: Client,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ViewState {
    #[default]
    Login,
    Register,
}

#[derive(Debug, Clone)]
pub enum Message {
    AccountChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    ConfirmPasswordChanged(String),
    SwitchView(ViewState),
    SubmitLogin,
    SubmitRegister,
    LoginResponse(LoginResponse),
    RegisterResponse(RegisterResponse),
}
pub enum Action {
    None,
    Run(Task<Message>),
    ChangeToChat {
        auth: Auth,
        client: Client,
        url: String,
    },
}

impl Login {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::AccountChanged(a) => {
                self.account = a;
                self.error = None;
            }
            Message::UsernameChanged(u) => {
                self.username = u;
                self.error = None;
            }
            Message::PasswordChanged(p) => {
                self.password = p;
                self.error = None;
            }
            Message::ConfirmPasswordChanged(cp) => {
                self.confirm_password = cp;
                self.error = None;
            }
            Message::SwitchView(view) => {
                self.view_state = view;
                self.account.clear();
                self.username.clear();
                self.password.clear();
                self.confirm_password.clear();
                self.error = None;
            }
            Message::SubmitLogin => {
                if self.account.is_empty() || self.password.is_empty() {
                    self.error = Some("账号和密码不能为空".to_string());
                    return Action::None;
                }

                println!("正在尝试登录: {}", self.account);
                let url = format!("{}login", self.inner.url);
                let l = SharedLogin {
                    account: self.account.clone(),
                    password: sha2::Sha256::digest(self.password.clone())
                        .as_slice()
                        .to_vec(),
                };
                let c = self.inner.client.clone();
                return Action::Run(Task::perform(
                    async move { crate::tools::auth::login(&c, &url, &l).await.unwrap() },
                    Message::LoginResponse,
                ));
            }
            Message::SubmitRegister => {
                if self.account.is_empty() || self.username.is_empty() || self.password.is_empty() {
                    self.error = Some("账号、用户名和密码不能为空".to_string());
                    return Action::None;
                }

                if self.password != self.confirm_password {
                    self.error = Some("两次输入的密码不一致".to_string());
                    return Action::None;
                }

                println!("正在尝试注册: {}, 用户名: {}", self.account, self.username);
                let url = format!("{}register", self.inner.url);
                let r = Register {
                    account: self.account.clone(),
                    user_name: self.username.clone(),
                    password: sha2::Sha256::digest(self.password.clone())
                        .as_slice()
                        .into(),
                };
                let c = self.inner.client.clone();
                return Action::Run(Task::perform(
                    async move { crate::tools::auth::register(&c, &url, &r).await.unwrap() },
                    Message::RegisterResponse,
                ));
            }
            Message::LoginResponse(r) => match r {
                LoginResponse::Success(s) => {
                    let auth = s.auth;
                    println!("login success!: {}", auth.token());
                    self.inner.auth = Some(auth.clone());
                    return Action::ChangeToChat {
                        auth,
                        client: self.inner.client.clone(),
                        url: self.inner.url.clone(),
                    };
                }
                LoginResponse::Fail(e) => {
                    println!("login failed: {:?}", e);
                    self.error = Some(format!("登录失败: {:?}", e));
                }
            },
            Message::RegisterResponse(r) => {
                match r {
                    RegisterResponse::Success(_) => {
                        println!("register success!");
                        // 注册成功后切换到登录页面
                        self.view_state = ViewState::Login;
                        self.account.clear();
                        self.username.clear();
                        self.password.clear();
                        self.confirm_password.clear();
                        self.error = Some("注册成功，请登录".to_string());
                    }
                    RegisterResponse::Fail(e) => {
                        println!("register failed: {:?}", e);
                        self.error = Some(format!("注册失败: {:?}", e));
                    }
                }
            }
        }
        Action::None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut content = column![]
            .spacing(20)
            .max_width(300)
            .align_x(Alignment::Center);

        let title = text(match self.view_state {
            ViewState::Login => "欢迎回来",
            ViewState::Register => "创建新账户",
        })
        .size(30);
        content = content.push(title);

        if self.view_state == ViewState::Register {
            let username_input = text_input("用户名", &self.username)
                .on_input(Message::UsernameChanged)
                .padding(10);
            content = content.push(username_input);
        }

        let account_input = text_input("账号", &self.account)
            .on_input(Message::AccountChanged)
            .padding(10);

        content = content.push(account_input);

        let password_input = text_input("密码", &self.password)
            .on_input(Message::PasswordChanged)
            .secure(true)
            .padding(10);

        content = content.push(password_input);

        if self.view_state == ViewState::Register {
            content = content.push(
                text_input("确认密码", &self.confirm_password)
                    .on_input(Message::ConfirmPasswordChanged)
                    .secure(true)
                    .padding(10),
            );
        }

        if let Some(error) = &self.error {
            content = content.push(text(error).size(14).color(iced::Color::from_rgb(0.8, 0.2, 0.2)));
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
impl Default for Login {
    fn default() -> Self {
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

        let server_name = std::env::var("SERVER_NAME").unwrap();
        let url = format!("https://{}/", server_name);
        let inner = Inner {
            auth: None,
            client,
            url,
        };
        Self {
            inner,
            view_state: ViewState::Login,
            account: String::new(),
            username: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            error: None,
        }
    }
}
