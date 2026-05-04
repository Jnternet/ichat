use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{account::AccountId, auth::Auth, group::GroupId};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct C2S_VC_Msg {
    pub target: GroupId,
    pub voice_data: Bytes,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S2C_VC_Msg {
    pub sender_id: AccountId,
    pub voice_data: Bytes,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VoiceGroupAuth {
    pub auth: Auth,
    pub gid: GroupId,
}
