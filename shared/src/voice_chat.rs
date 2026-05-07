use rkyv::{Archive, Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::Auth;

#[allow(non_camel_case_types)]
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
pub struct C2S_VC_Msg {
    pub voice_data: Vec<f32>,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
pub struct S2C_VC_Msg {
    pub sender_id: Uuid,
    pub voice_data: Vec<f32>,
}

#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
pub struct VoiceGroupAuth {
    pub auth: Auth,
    pub gid: Uuid,
}
