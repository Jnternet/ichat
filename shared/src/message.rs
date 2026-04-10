use serde::{Deserialize, Serialize};

use crate::{account::OtherUser, auth::Auth, group::GroupId};
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

    pub fn new(auth: Auth, target: GroupId, msg: Msg) -> Self {
        Self { auth, target, msg }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S2C_Msg {
    sender: OtherUser,
    msg: Msg,
}

impl S2C_Msg {
    pub fn new(sender: OtherUser, msg: Msg) -> Self {
        Self { sender, msg }
    }
    pub fn sender(&self) -> &OtherUser {
        &self.sender
    }

    pub fn msg(&self) -> &Msg {
        &self.msg
    }
    pub fn sender_name(&self) -> &str {
        self.sender.user_name()
    }
}
