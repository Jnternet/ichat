use crate::tools;
use chat_util::{OneMessage, UIGroups, get_group_messages, get_groups_info};
use iced::Subscription;
use iced::futures::SinkExt;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Task};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;
use shared::chrono;
use shared::group::GroupId;
use shared::message::{C2S_Msg, Msg};
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::tools::textchat::text_chat;

mod chat_util;

#[derive(Default)]
pub struct Chat {
    pub inner: Option<Inner>,
    groups: Option<UIGroups>,
    selected_group: Option<GroupId>,
    messages: Vec<OneMessage>,
    input: String,
}

pub struct Inner {
    auth: Auth,
    db: DatabaseConnection,
    client: Client,
    url: String,
    text_sender: Sender<C2S_Msg>,
    subs_recv: HashRx,
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
    EmptyRedraw,
}

// UIGroups 不能 derive Clone，手动实现
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
        let (s, r) = tokio::sync::mpsc::channel(100);
        let (s2, r2) = tokio::sync::mpsc::channel(100);
        let inner = Inner {
            auth: auth.clone(),
            db: db.clone(),
            client: client.clone(),
            url: url.clone(),
            text_sender: s,
            subs_recv: HashRx::new(r2),
        };
        let db_ = db.clone();
        let auth_ = auth.clone();
        tokio::spawn(async move {
            let gu = tools::update_info::get_last_message_timestamp(&db_, &auth_)
                .await
                .unwrap();
            let uir = tools::update_info::update_info(&client, &url, &gu)
                .await
                .unwrap();
            let nm = uir.success().unwrap();
            tools::update_info::save_to_db(&db_, &client, &url, nm, &auth_)
                .await
                .unwrap();
        });
        let db_ = db.clone();
        let auth_ = auth.clone();
        tokio::spawn(async move {
            text_chat(auth_, db_, r, s2).await.unwrap();
        });
        let chat = Self {
            inner: Some(inner),
            ..Default::default()
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
                let Some(gid) = &self.selected_group else {
                    eprintln!("[chat] SendMessage: 未选择群组");
                    return Action::None;
                };
                let Some(inner) = &self.inner else {
                    eprintln!("[chat] SendMessage: inner 为空");
                    return Action::None;
                };
                if self.input.is_empty() {
                    eprintln!("[chat] SendMessage: 输入为空，放弃发送");
                    return Action::None;
                }
                eprintln!("[chat] SendMessage: 发送消息到群组 {:?}，内容: {:?}", gid, self.input);
                let auth = inner.auth.clone();
                let msg = Msg::new(self.input.clone());
                let now = chrono::Utc::now();
                let c2s_msg = C2S_Msg::new(auth.clone(), *gid, msg, now);

                let db = inner.db.clone();
                let gid = *gid;

                self.input.clear();
                let s_ = inner.text_sender.clone();
                Action::Run(Task::perform(
                    async move {
                        eprintln!("[chat] 将消息写入 channel");
                        match s_.send(c2s_msg).await {
                            Ok(_) => eprintln!("[chat] 消息写入 channel 成功"),
                            Err(e) => eprintln!("[chat] 消息写入 channel 失败: {:?}", e),
                        }
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
                self.groups = Some(g);
                self.messages = m;
                Action::None
            }
            Message::EmptyRedraw => {
                let Some(gid) = &self.selected_group else {
                    return Action::None;
                };
                let Some(inner) = &self.inner else {
                    return Action::None;
                };
                let auth = inner.auth.clone();

                let db = inner.db.clone();
                let gid = *gid;
                Action::Run(Task::perform(
                    async move { redraw(&gid, auth, db).await },
                    Message::Redraw,
                ))
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let group_list = self.view_group_list();
        let chat_area = self.view_chat_area();

        row![group_list, chat_area]
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

                let item = column![text(&g.name).size(15), text(preview).size(12),].spacing(2);

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

        let input_row = row![
            text_input("输入消息...", &self.input)
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .padding(8)
                .width(Length::Fill),
            button("发送").on_press(Message::SendMessage).padding(8),
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

    // pub fn subscription(&self) -> Subscription<Message> {
    //     let Some(inner) = &self.inner else {
    //         return Subscription::none();
    //     };
    //     if let Ok(mut opt) = inner.subs_recv.try_borrow_mut() {
    //         if let Some(rx) = opt.take() {
    //             let r = Arc::new(HashRx::new(rx));
    //             // TODO: 创建
    //             return Subscription::run_with((r,), move |(hr,)| {
    //                 let ar = hr.clone();
    //                 iced::stream::channel(
    //                     100,
    //                     move |mut out: iced::futures::channel::mpsc::Sender<Message>| async move {
    //                         while let Some(()) = ar.1.clone().lock().await.recv().await {
    //                             out.send(Message::EmptyRedraw).await.unwrap();
    //                         }
    //                     },
    //                 )
    //             });
    //         }
    //         return Subscription::none();
    //     }
    //     Subscription::none()
    // }
    pub fn subscription(&self) -> Subscription<Message> {
        use iced::futures::channel::mpsc::Sender as IcedSender;
        let Some(inner) = &self.inner else {
            return Subscription::none();
        };
        let ar = inner.subs_recv.clone();
        Subscription::run_with(ar, move |ar| {
            let r = ar.clone();
            iced::stream::channel(100, move |mut out: IcedSender<Message>| async move {
                eprintln!("[chat] subscription 已启动，等待服务器消息");
                while let Some(()) = r.1.clone().lock().await.recv().await {
                    eprintln!("[chat] 收到服务器消息通知，触发 EmptyRedraw");
                    out.send(Message::EmptyRedraw).await.unwrap();
                }
                eprintln!("[chat] subscription channel 已关闭");
            })
        })
    }
}
#[derive(Debug, Clone)]
struct HashRx(Uuid, Arc<Mutex<Receiver<()>>>);
impl Hash for HashRx {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl HashRx {
    fn new(r: Receiver<()>) -> Self {
        Self(Uuid::new_v4(), Arc::new(Mutex::new(r)))
    }
}

async fn redraw(gid: &GroupId, auth: Auth, db: DatabaseConnection) -> (UIGroups, Vec<OneMessage>) {
    let g = get_groups_info(auth.clone(), db.clone()).await.unwrap();
    let m = get_group_messages(auth, db, *gid).await.unwrap();
    (g, m)
}
