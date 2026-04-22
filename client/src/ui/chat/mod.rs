use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Subscription, Task};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;
use shared::group::GroupId;
use shared::message::{C2S_Msg, Msg};

use iced_futures::subscription::{EventStream, Hasher, Recipe, from_recipe};
use iced_futures::BoxStream;

mod chat_util;
use chat_util::{OneMessage, TlsWriteHalf, UIGroups, connect, get_group_messages, get_groups_info};

pub struct Chat {
    pub inner: Option<Inner>,
    groups: Option<UIGroups>,
    selected_group: Option<GroupId>,
    messages: Vec<OneMessage>,
    input: String,
    write_half: Option<TlsWriteHalf>,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            inner: None,
            groups: None,
            selected_group: None,
            messages: Vec::new(),
            input: String::new(),
            write_half: None,
        }
    }
}

#[derive(Clone)]
pub struct Inner {
    pub auth: Auth,
    pub db: DatabaseConnection,
    pub client: Client,
    pub url: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    GroupsLoaded(Result<UIGroups, String>),
    SelectGroup(GroupId),
    MessagesLoaded(Result<Vec<OneMessage>, String>),
    InputChanged(String),
    SendMessage,
    Connected(Result<TlsWriteHalf, String>),
    // 收到新消息并存入 DB，携带所属群组 id
    Refresh(GroupId),
    Exit,
}

impl Clone for UIGroups {
    fn clone(&self) -> Self {
        unreachable!("UIGroups should not be cloned")
    }
}
impl std::fmt::Debug for UIGroups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UIGroups")
    }
}
impl std::fmt::Debug for OneMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OneMessage({})", self.content)
    }
}
impl Clone for OneMessage {
    fn clone(&self) -> Self {
        OneMessage { content: self.content.clone(), is_mine: self.is_mine, time: self.time }
    }
}

pub enum Action {
    None,
    Run(Task<Message>),
    ChangeToLogin { client: Client, url: String },
}

struct TcpRecipe {
    account_id: uuid::Uuid,
    auth: Auth,
    db: DatabaseConnection,
}

impl Recipe for TcpRecipe {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        self.account_id.hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let auth = self.auth;
        let db = self.db;

        Box::pin(iced::stream::channel(64, async move |mut output| {
            use iced::futures::SinkExt;

            // 建立 TLS 连接
            let (wh, rh) = match connect(&auth).await {
                Ok(v) => v,
                Err(e) => {
                    let _ = output.send(Message::Connected(Err(e.to_string()))).await;
                    return;
                }
            };
            let _ = output.send(Message::Connected(Ok(wh))).await;

            // 用 tokio mpsc 把 rh 的消息传给 output
            // output 是 futures::channel::mpsc::Sender，不能跨线程 clone
            // 所以直接在同一个 async 块里循环读取
            use tokio::io::AsyncReadExt;
            let mut rh = rh;
            let mut buf = bytes::BytesMut::with_capacity(4096);
            loop {
                buf.clear();
                match rh.read_buf(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        match shared::serde_json::from_slice::<shared::message::S2C_Msg>(
                            &buf[..n],
                        ) {
                            Ok(s2c) => {
                                let group_id = *s2c.target();
                                if crate::tools::update_info::save_msg(&db, &s2c).await.is_ok() {
                                    let _ = output.send(Message::Refresh(group_id)).await;
                                }
                            }
                            Err(e) => eprintln!("parse msg error: {}", e),
                        }
                    }
                }
            }
        }))
    }
}

impl Chat {
    pub fn new(
        auth: Auth,
        db: DatabaseConnection,
        client: Client,
        url: String,
    ) -> (Self, Task<Message>) {
        let inner = Inner { auth, db, client, url };
        let chat = Self { inner: Some(inner), ..Default::default() };
        let task = chat.load_groups_task();
        (chat, task)
    }

    fn load_groups_task(&self) -> Task<Message> {
        let Some(inner) = &self.inner else { return Task::none() };
        let auth = inner.auth.clone();
        let db = inner.db.clone();
        Task::perform(
            async move { get_groups_info(auth, db).await },
            Message::GroupsLoaded,
        )
    }

    fn reload_messages_task(&self) -> Task<Message> {
        let Some(group_id) = self.selected_group else { return Task::none() };
        let Some(inner) = &self.inner else { return Task::none() };
        let auth = inner.auth.clone();
        let db = inner.db.clone();
        Task::perform(
            async move { get_group_messages(auth, db, group_id).await },
            Message::MessagesLoaded,
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some(inner) = &self.inner else { return Subscription::none() };
        from_recipe(TcpRecipe {
            account_id: inner.auth.account_id(),
            auth: inner.auth.clone(),
            db: inner.db.clone(),
        })
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::GroupsLoaded(Ok(groups)) => {
                self.groups = Some(groups);
                Action::None
            }
            Message::GroupsLoaded(Err(e)) => {
                eprintln!("Failed to load groups: {}", e);
                Action::None
            }
            Message::SelectGroup(group_id) => {
                self.selected_group = Some(group_id);
                self.messages.clear();
                Action::Run(self.reload_messages_task())
            }
            Message::MessagesLoaded(Ok(msgs)) => {
                self.messages = msgs;
                Action::None
            }
            Message::MessagesLoaded(Err(e)) => {
                eprintln!("Failed to load messages: {}", e);
                Action::None
            }
            Message::InputChanged(s) => {
                self.input = s;
                Action::None
            }
            Message::SendMessage => {
                let input = self.input.trim().to_string();
                if input.is_empty() {
                    return Action::None;
                }
                let Some(group_id) = self.selected_group else { return Action::None };
                let Some(wh) = self.write_half.clone() else { return Action::None };
                let Some(inner) = &self.inner else { return Action::None };
                let auth = inner.auth.clone();
                self.input.clear();
                Action::Run(Task::perform(
                    async move {
                        use tokio::io::AsyncWriteExt;
                        let c2s = C2S_Msg::new(
                            auth,
                            group_id,
                            Msg::new(input),
                            shared::chrono::Utc::now(),
                        );
                        let bytes =
                            shared::serde_json::to_vec(&c2s).map_err(|e| e.to_string())?;
                        let mut wh = wh.0.lock().await;
                        wh.write_all(&bytes).await.map_err(|e| e.to_string())?;
                        wh.flush().await.map_err(|e| e.to_string())?;
                        Ok::<(), String>(())
                    },
                    |r| {
                        if let Err(e) = r {
                            eprintln!("Send failed: {}", e);
                        }
                        Message::InputChanged(String::new())
                    },
                ))
            }
            Message::Connected(Ok(wh)) => {
                self.write_half = Some(wh);
                Action::None
            }
            Message::Connected(Err(e)) => {
                eprintln!("Connection failed: {}", e);
                Action::None
            }
            Message::Refresh(group_id) => {
                // 始终刷新群组列表（更新 last_msg）
                // 只有收到消息的群组是当前选中群组时才刷新消息列表
                let msg_task = if self.selected_group == Some(group_id) {
                    self.reload_messages_task()
                } else {
                    Task::none()
                };
                Action::Run(Task::batch([self.load_groups_task(), msg_task]))
            }
            Message::Exit => {
                let Some(inner) = &self.inner else { return Action::None };
                Action::ChangeToLogin {
                    client: inner.client.clone(),
                    url: inner.url.clone(),
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        row![self.view_group_list(), self.view_chat_area()]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_group_list(&self) -> Element<'_, Message> {
        let mut col = column![row![
            text("群组").size(18).width(Length::Fill),
            button("退出").on_press(Message::Exit)
        ]
        .align_y(Alignment::Center)
        .padding(10)]
        .spacing(0);

        if let Some(groups) = &self.groups {
            for g in &groups.groups {
                let is_selected = self.selected_group == Some(g.id);
                let preview = g
                    .last_msg
                    .as_deref()
                    .unwrap_or("暂无消息")
                    .chars()
                    .take(20)
                    .collect::<String>();
                let item = column![text(&g.name).size(15), text(preview).size(12)].spacing(2);
                let btn = button(item)
                    .on_press(Message::SelectGroup(g.id))
                    .width(Length::Fill)
                    .padding(8);
                col = col.push(if is_selected {
                    container(btn).style(container::rounded_box)
                } else {
                    container(btn)
                });
            }
        } else {
            col = col.push(text("加载中...").size(14));
        }

        container(scrollable(col))
            .width(220)
            .height(Length::Fill)
            .style(container::bordered_box)
            .into()
    }

    fn view_chat_area(&self) -> Element<'_, Message> {
        if self.selected_group.is_none() {
            return container(text("请选择一个群组").size(16))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }

        let mut msg_col = column![].spacing(8).padding(10);
        for msg in &self.messages {
            let bubble = container(text(&msg.content).size(14))
                .padding(8)
                .style(container::rounded_box);
            let row_item = if msg.is_mine {
                row![iced::widget::Space::new().width(Length::Fill), bubble]
            } else {
                row![bubble, iced::widget::Space::new().width(Length::Fill)]
            };
            msg_col = msg_col.push(row_item.width(Length::Fill));
        }

        let connected = self.write_half.is_some();
        let input_row = row![
            text_input("输入消息...", &self.input)
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .padding(8)
                .width(Length::Fill),
            button(if connected { "发送" } else { "连接中..." })
                .on_press_maybe(connected.then_some(Message::SendMessage))
                .padding(8),
        ]
        .spacing(8)
        .padding(10)
        .align_y(Alignment::Center);

        column![
            scrollable(msg_col).height(Length::Fill).width(Length::Fill),
            input_row,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
