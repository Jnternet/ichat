use serde::{Deserialize, Serialize};

use crate::{account::OtherUser, auth::Auth, group::GroupId};
#[derive(Debug, Serialize, Deserialize)]
pub struct Msg {
    text: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize)]
pub struct C2S_Msg {
    auth: Auth,
    target: GroupId,
    msg: Msg,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize)]
pub struct S2C_Msg {
    sender: OtherUser,
    msg: Msg,
}
