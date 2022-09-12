extern crate websocket;

mod chat;
mod protocol;

use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;

use chat::ChatMessage;
use rand::distributions::{Alphanumeric, DistString};

use websocket::sync::{Server, Writer};
use websocket::OwnedMessage;

use crate::chat::ChatClient;
use crate::protocol::{ClientBoundPacket, ServerBoundPacket};

fn main() {
    // Create shat server
    let chat_server = Arc::new(ChatServer::new());

    // Start web socket server
    let server = Server::bind("127.0.0.1:8000").unwrap();

    for connection in server.filter_map(Result::ok) {
        // Define arc copies that can be moved into the thread safely
        let thread_chat_server = Arc::clone(&chat_server);

        // Spawn a new thread for each connection.
        thread::spawn(move || {
            // Get the client info
            let client = connection.accept().unwrap();

            let ip = client.peer_addr().unwrap();
            println!("New client connection: {:?}", ip);

            let (mut receiver, sender) = client.split().unwrap();

            // Create a chat client with a random username (can be changed later)
            thread_chat_server.add_client(ip, sender);

            // Listen to incoming messages
            for message in receiver.incoming_messages() {
                if let Err(err) = message {
                    println!("Got WebSocket error instead of message: {:?}", err);
                    thread_chat_server.remove_client(&ip);
                    break;
                }

                match message.unwrap() {
                    OwnedMessage::Close(_) => {
                        thread_chat_server.send_message(&ip, OwnedMessage::Close(None));
                        thread_chat_server.remove_client(&ip);
                        println!("Client {} disconnected", ip);
                        return;
                    }
                    OwnedMessage::Ping(ping) => {
                        thread_chat_server.send_message(&ip, OwnedMessage::Pong(ping));
                    }
                    OwnedMessage::Text(text) => {
                        let data: Result<ServerBoundPacket, _> = serde_json::from_str(&text);

                        if let Err(_) = data {
                            println!("Got malformed packet: {}", text);
                            break;
                        }

                        match data.unwrap() {
                            ServerBoundPacket::Message { text } => {
                                // Create the message struct
                                let chat_message = ChatMessage {
                                    text,
                                    username: thread_chat_server.get_username(&ip),
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis(),
                                };

                                // Store chat message in database
                                thread_chat_server.insert_message_into_db(&chat_message);

                                thread_chat_server.emit_message(ClientBoundPacket::Message {
                                    text: chat_message.text,
                                    username: chat_message.username,
                                    timestamp: chat_message.timestamp,
                                });
                            }
                            ServerBoundPacket::SetUsername { username } => {
                                if username.len() > 12 {
                                    break;
                                }

                                thread_chat_server.set_username(&ip, username);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

fn create_random_username() -> String {
    format!(
        "user{}",
        Alphanumeric
            .sample_string(&mut rand::thread_rng(), 4)
            .to_uppercase()
    )
}

type SQLiteConnection = Arc<Mutex<sqlite::Connection>>;
type ChatClients = Arc<Mutex<HashMap<SocketAddr, RwLock<ChatClient>>>>;

struct ChatServer {
    pub chat_clients: ChatClients,
    pub sqlite_conn: SQLiteConnection,
}

impl ChatServer {
    pub fn new() -> ChatServer {
        let server = ChatServer {
            chat_clients: Arc::new(Mutex::new(HashMap::new())),
            sqlite_conn: Arc::new(Mutex::new(sqlite::open("./data.sqlite").unwrap())),
        };

        server.init_sqlite();

        server
    }

    pub fn emit_message(&self, packet: ClientBoundPacket) {
        let clients = self.chat_clients.lock().unwrap();

        // Create message
        let serialized = serde_json::to_string(&packet).unwrap();

        let message = OwnedMessage::Text(serialized);

        for (_, client) in clients.iter() {
            let mut client = client.write().unwrap();

            client.sender.send_message(&message).unwrap();
        }
    }

    pub fn add_client(&self, ip: SocketAddr, sender: Writer<TcpStream>) {
        let mut clients = self.chat_clients.lock().unwrap();

        let username = create_random_username();

        let chat_client = ChatClient {
            ip,
            username: create_random_username(),
            sender,
        };

        clients.insert(ip, RwLock::new(chat_client));

        // Unlock clients for iterator
        drop(clients);

        // Emit client join
        self.emit_message(ClientBoundPacket::ClientJoin {
            username: username.clone(),
        })
    }

    pub fn remove_client(&self, ip: &SocketAddr) {
        let mut clients = self.chat_clients.lock().unwrap();

        let client = clients.remove(ip).unwrap();
        let client = client.read().unwrap();

        // Unlock clients for iterator
        drop(clients);

        // Emit client join
        self.emit_message(ClientBoundPacket::ClientLeave {
            username: client.username.clone(),
        })
    }

    pub fn get_username(&self, ip: &SocketAddr) -> String {
        let clients = self.chat_clients.lock().unwrap();

        let client = clients.get(ip).unwrap();
        let client = client.read().unwrap();

        client.username.clone()
    }

    pub fn set_username(&self, ip: &SocketAddr, username: String) {
        let clients = self.chat_clients.lock().unwrap();

        let client = clients.get(ip).unwrap();
        let mut client = client.write().unwrap();

        client.username = username
    }

    // pub fn send_packet(&self, ip: &SocketAddr, packet: ClientBoundPacket) {
    //     let serialized = serde_json::to_string(&packet).unwrap();

    //     let message = OwnedMessage::Text(serialized);

    //     self.send_message(ip, message)
    // }

    pub fn send_message(&self, ip: &SocketAddr, message: OwnedMessage) {
        let clients = self.chat_clients.lock().unwrap();

        let chat_client = clients.get(&ip).unwrap();
        let mut chat_client = chat_client.write().unwrap();

        chat_client.sender.send_message(&message).unwrap();
    }

    fn init_sqlite(&self) {
        let conn = self.sqlite_conn.lock().unwrap();

        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS chat_messages (text TEXT, username VARCHAR(16), timestamp INTEGER);
            ",
        )
        .unwrap();
    }

    pub fn insert_message_into_db(&self, message: &ChatMessage) {
        let conn = self.sqlite_conn.lock().unwrap();

        let query = format!(
            "INSERT INTO chat_messages (text, username, timestamp) VALUES ('{}', '{}', '{}');",
            &message.text, &message.username, &message.timestamp
        );

        conn.execute(query).unwrap();
    }
}
