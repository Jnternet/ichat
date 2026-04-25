use crate::tools;
use crate::tools::textchat::{get_connector, get_tls_stream};
use crate::tools::update_info::save_msg;
use chat_util::{OneMessage, UIGroups, get_group_messages, get_groups_info};
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender as IcedSender;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Subscription, Task, Color};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;
use shared::chrono;
use shared::group::GroupId;
use shared::message::{C2S_Msg, Msg, S2C_Msg};
use shared::serde_json;
use std::hash::{Hash, Hasher};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

mod chat_util;

// ── subscription 的 data 类型，实现 Hash 供 run_with 去重 ──────────────────

#[derive(Clone)]
struct SubData {
    auth: Auth,
    db: DatabaseConnection,
}

impl Hash for SubData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.auth.account_id().hash(state);
    }
}

impl PartialEq for SubData {
    fn eq(&self, other: &Self) -> bool {
        self.auth.account_id() == other.auth.account_id()
    }
}

impl Eq for SubData {}

// builder 必须是 fn 指针（不捕获），从 &SubData 取数据
fn textchat_stream(data: &SubData) -> iced::futures::stream::BoxStream<'static, Message> {
    let auth = data.auth.clone();
    let _db = data.db.clone();

    Box::pin(iced::stream::channel(
        100,
        move |mut output: IcedSender<Message>| async move {
            // 建立内部 channel：stream 持有 rx，tx 通过 Message::Ready 交给 update()
            let (tx, mut rx) = mpsc::channel::<C2S_Msg>(100);
            if output.send(Message::Ready(tx)).await.is_err() {
                return;
            }

            let server_addr = match std::env::var("SERVER_TEXTCHAT_ADDR") {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[sub] SERVER_TEXTCHAT_ADDR: {e}");
                    return;
                }
            };
            let server_name = match std::env::var("SERVER_NAME") {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[sub] SERVER_NAME: {e}");
                    return;
                }
            };

            let connector = get_connector();
            let mut tls = match get_tls_stream(&connector, &server_addr, &server_name).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[sub] TLS 连接失败: {e}");
                    return;
                }
            };

            let auth_bytes = serde_json::to_vec(&auth).unwrap();
            if tls.write_all(&auth_bytes).await.is_err() || tls.flush().await.is_err() {
                eprintln!("[sub] 发送 Auth 失败");
                return;
            }

            let (mut rh, mut wh) = tokio::io::split(tls);
            let mut buf = bytes::BytesMut::with_capacity(4096);

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        let Some(c2s) = msg else { break; };
                        let b = serde_json::to_vec(&c2s).unwrap();
                        if wh.write_all(&b).await.is_err() || wh.flush().await.is_err() {
                            eprintln!("[sub] 发送消息失败");
                            break;
                        }
                    }
                    result = rh.read_buf(&mut buf) => {
                        match result {
                            Ok(0) => { eprintln!("[sub] 服务器关闭连接"); break; }
                            Ok(_) => {
                                match serde_json::from_slice::<S2C_Msg>(&buf) {
                                    Ok(msg) => {
                                        buf.clear();
                                        let _ = output.send(Message::ServerMsg(msg)).await;
                                    }
                                    Err(e) => {
                                        eprintln!("[sub] 解析失败 ({} bytes): {e}", buf.len());
                                    }
                                }
                            }
                            Err(e) => { eprintln!("[sub] 读取失败: {e}"); break; }
                        }
                    }
                }
            }
        },
    ))
}

// ── Chat ──────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Chat {
    pub inner: Option<Inner>,
    groups: Option<UIGroups>,
    selected_group: Option<GroupId>,
    messages: Vec<OneMessage>,
    input: String,
    last_message_count: usize,
}

pub struct Inner {
    auth: Auth,
    db: DatabaseConnection,
    client: Client,
    url: String,
    msg_tx: Option<mpsc::Sender<C2S_Msg>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    GroupsLoaded(Result<UIGroups, String>),
    SelectGroup(GroupId),
    MessagesLoaded(Result<Vec<OneMessage>, String>),
    InputChanged(String),
    SendMessage,
    Exit,
    Redraw((UIGroups, Vec<OneMessage>)),
    Ready(mpsc::Sender<C2S_Msg>),
    ServerMsg(S2C_Msg),
    ScrollToBottom,
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
        OneMessage {
            content: self.content.clone(),
            is_mine: self.is_mine,
            time: self.time,
            sender_name: self.sender_name.clone(),
        }
    }
}

pub enum Action {
    None,
    Run(Task<Message>),
    ChangeToLogin { client: Client, url: String },
}

impl Chat {
    pub fn new(
        auth: Auth,
        db: DatabaseConnection,
        client: Client,
        url: String,
    ) -> (Self, Task<Message>) {
        let db_ = db.clone();
        let auth_ = auth.clone();
        let client_ = client.clone();
        let url_ = url.clone();
        tokio::spawn(async move {
            let gu = tools::update_info::get_last_message_timestamp(&db_, &auth_)
                .await
                .unwrap();
            let uir = tools::update_info::update_info(&client_, &url_, &gu)
                .await
                .unwrap();
            let nm = uir.success().unwrap();
            tools::update_info::save_to_db(&db_, &client_, &url_, nm, &auth_)
                .await
                .unwrap();
        });

        let inner = Inner {
            auth: auth.clone(),
            db: db.clone(),
            client,
            url,
            msg_tx: None,
        };
        let chat = Self {
            inner: Some(inner),
            groups: None,
            selected_group: None,
            messages: Vec::new(),
            input: String::new(),
            last_message_count: 0,
        };
        let task = chat.load_groups_task();
        (chat, task)
    }

    fn load_groups_task(&self) -> Task<Message> {
        let Some(inner) = &self.inner else {
            return Task::none();
        };
        let auth = inner.auth.clone();
        let db = inner.db.clone();
        Task::perform(
            async move { get_groups_info(auth, db).await },
            Message::GroupsLoaded,
        )
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
                let Some(inner) = &self.inner else {
                    return Action::None;
                };
                let auth = inner.auth.clone();
                let db = inner.db.clone();
                Action::Run(Task::perform(
                    async move { get_group_messages(auth, db, group_id).await },
                    Message::MessagesLoaded,
                ))
            }
            Message::MessagesLoaded(Ok(msgs)) => {
                let had_more_messages = msgs.len() > self.last_message_count;
                self.messages = msgs;
                self.last_message_count = self.messages.len();
                if had_more_messages && self.selected_group.is_some() {
                    return Action::Run(Task::done(Message::ScrollToBottom));
                }
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
                let Some(gid) = self.selected_group else {
                    return Action::None;
                };
                let Some(inner) = &self.inner else {
                    return Action::None;
                };
                if self.input.is_empty() {
                    return Action::None;
                }
                let Some(tx) = inner.msg_tx.clone() else {
                    eprintln!("[chat] subscription 尚未就绪");
                    return Action::None;
                };
                let auth = inner.auth.clone();
                let msg = Msg::new(self.input.clone());
                let now = chrono::Utc::now();
                let c2s_msg = C2S_Msg::new(auth.clone(), gid, msg, now);
                let db = inner.db.clone();
                self.input.clear();
                Action::Run(Task::perform(
                    async move {
                        let _ = tx.send(c2s_msg).await;
                        redraw(&gid, auth, db).await
                    },
                    Message::Redraw,
                ))
            }
            Message::Exit => {
                let Some(inner) = &self.inner else {
                    return Action::None;
                };
                Action::ChangeToLogin {
                    client: inner.client.clone(),
                    url: inner.url.clone(),
                }
            }
            Message::Redraw((g, m)) => {
                let had_more_messages = m.len() > self.last_message_count;
                self.groups = Some(g);
                self.messages = m;
                self.last_message_count = self.messages.len();
                if had_more_messages && self.selected_group.is_some() {
                    return Action::Run(Task::done(Message::ScrollToBottom));
                }
                Action::None
            }
            Message::ScrollToBottom => {
                Action::None
            }
            Message::Ready(tx) => {
                if let Some(inner) = &mut self.inner {
                    inner.msg_tx = Some(tx);
                }
                Action::None
            }
            Message::ServerMsg(s2c_msg) => {
                let Some(inner) = &self.inner else {
                    return Action::None;
                };
                let db = inner.db.clone();
                let auth = inner.auth.clone();
                let gid = self.selected_group;
                Action::Run(Task::perform(
                    async move {
                        let _ = save_msg(&db, &s2c_msg).await;
                        match gid {
                            Some(gid) => redraw(&gid, auth, db).await,
                            None => {
                                let g = get_groups_info(auth, db).await.unwrap();
                                (g, vec![])
                            }
                        }
                    },
                    Message::Redraw,
                ))
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
        let mut col = column![
            row![
                text("群组").size(18).width(Length::Fill),
                button("退出").on_press(Message::Exit)
            ]
            .align_y(Alignment::Center)
            .padding(10)
        ]
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
                let item = column![
                    text(&g.name).size(16).color(Color::from_rgb(0.0, 0.0, 0.0)),
                    text(preview).size(13).color(Color::from_rgb(0.3, 0.3, 0.3))
                ].spacing(4);
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
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgb(0.92, 0.92, 0.92))),
                ..Default::default()
            })
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

        let mut msg_col = column![].spacing(12).padding(15);
        for msg in &self.messages {
            let bubble = container(
                text(&msg.content)
                    .size(15)
                    .line_height(1.5)
            )
            .padding(12)
            .style(container::rounded_box);
            let row_item = if msg.is_mine {
                row![iced::widget::Space::new().width(Length::Fill), bubble]
            } else {
                let name_label = text(&msg.sender_name)
                    .size(12)
                    .color(Color::from_rgb(0.2, 0.6, 0.3));
                let with_name = column![name_label, bubble].spacing(4);
                row![with_name, iced::widget::Space::new().width(Length::Fill)]
            };
            msg_col = msg_col.push(row_item.width(Length::Fill));
        }

        let input_row = row![
            text_input("输入消息...", &self.input)
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .padding(10)
                .size(15)
                .width(Length::Fill),
            button("发送")
                .on_press(Message::SendMessage)
                .padding(10)
        ]
        .spacing(10)
        .padding(12)
        .align_y(Alignment::Center);

        column![
            scrollable(msg_col)
                .height(Length::Fill)
                .width(Length::Fill)
                .anchor_bottom(),
            input_row,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some(inner) = &self.inner else {
            return Subscription::none();
        };
        Subscription::run_with(
            SubData {
                auth: inner.auth.clone(),
                db: inner.db.clone(),
            },
            textchat_stream,
        )
    }
}

async fn redraw(gid: &GroupId, auth: Auth, db: DatabaseConnection) -> (UIGroups, Vec<OneMessage>) {
    let g = get_groups_info(auth.clone(), db.clone()).await.unwrap();
    let m = get_group_messages(auth, db, *gid).await.unwrap();
    (g, m)
}
