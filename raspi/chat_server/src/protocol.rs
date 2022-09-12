use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ServerBoundPacket {
    // Server bound
    Message { text: String },
    SetUsername { username: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientBoundPacket {
    // Client bound
    Message {
        text: String,
        username: String,
        timestamp: u128,
    },
    ClientJoin {
        username: String,
    },
    ClientLeave {
        username: String,
    },
    ClientTyping {
        username: String,
    },
}
