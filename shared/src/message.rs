use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    account::{AccountId, OtherUser, UserInfo},
    auth::Auth,
    group::GroupId,
};
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Msg {
    text: String,
}

impl Msg {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn new(text: String) -> Self {
        Self { text }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct C2S_Msg {
    auth: Auth,
    target: GroupId,
    msg: Msg,
    time: DateTime<Utc>,
}

impl C2S_Msg {
    pub fn auth(&self) -> &Auth {
        &self.auth
    }

    pub fn target(&self) -> &GroupId {
        &self.target
    }

    pub fn msg(&self) -> &Msg {
        &self.msg
    }

    pub fn new(auth: Auth, target: GroupId, msg: Msg, time: DateTime<Utc>) -> Self {
        Self {
            auth,
            target,
            msg,
            time,
        }
    }
    pub fn time(&self) -> DateTime<Utc> {
        self.time
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S2C_Msg {
    sender: UserInfo,
    target: GroupId,
    msg: Msg,
    time: DateTime<Utc>,
}

impl S2C_Msg {
    pub fn new(sender: UserInfo, msg: Msg, target: GroupId, time: DateTime<Utc>) -> Self {
        Self {
            sender,
            msg,
            target,
            time,
        }
    }
    pub fn sender(&self) -> &UserInfo {
        &self.sender
    }

    pub fn msg(&self) -> &Msg {
        &self.msg
    }
    pub fn sender_name(&self) -> &str {
        self.sender.user_name()
    }
    pub fn target(&self) -> &GroupId {
        &self.target
    }
}
