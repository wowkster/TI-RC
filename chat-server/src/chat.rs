use std::net::SocketAddr;

use websocket::stream::sync::TcpStream;
use websocket::sync::Writer;

pub struct ChatClient {
    pub ip: SocketAddr,
    pub username: String,
    pub sender: Writer<TcpStream>
}

pub struct ChatMessage {
    pub text: String,
    pub username: String,
    pub timestamp: u128,
}
