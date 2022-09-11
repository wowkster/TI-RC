use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageS2CData {
    pub text: String,
    pub username: String,
    pub timestamp: u128,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientJoinS2CData {
    pub client_id: usize,
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientLeaveS2CData {
    pub client_id: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientTypingS2CData {
    pub client_id: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageC2SData {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetUsernameC2SData {
    pub username: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProtocolEvent {
    // Client bound
    MessageS2C(MessageS2CData),
    ClientJoinS2C(ClientJoinS2CData),
    ClientLeaveS2C(ClientLeaveS2CData),
    ClientTypingS2C(ClientTypingS2CData),
    // Server bound
    MessageC2S(MessageC2SData),
    SetUsernameC2S(SetUsernameC2SData),
}
