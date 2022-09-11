extern crate websocket;

mod chat;
mod protocol;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::SystemTime;

use chat::ChatMessage;
use rand::distributions::{Alphanumeric, DistString};

use websocket::sync::Server;
use websocket::OwnedMessage;

use crate::chat::ChatClient;
use crate::protocol::{MessageS2CData, ProtocolEvent};

type SQLiteConnection = Arc<Mutex<sqlite::Connection>>;

fn main() {
    // Initialize Database
    let sqlite_connection = Arc::new(Mutex::new(sqlite::open("./data.sqlite").unwrap()));

    init_sqlite(&sqlite_connection);

    // Start web socket server
    let server = Server::bind("127.0.0.1:8000").unwrap();

    let chat_clients: Arc<Mutex<HashMap<SocketAddr, RwLock<ChatClient>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for connection in server.filter_map(Result::ok) {
        // Define arc copies that can be moved into the thread safely
        let thread_chat_clients = Arc::clone(&chat_clients);
        let thread_sqlite = Arc::clone(&sqlite_connection);

        // Spawn a new thread for each connection.
        thread::spawn(move || {
            // Reject connections that don't support the protocol
            if !connection
                .protocols()
                .contains(&"rust-websocket".to_string())
            {
                connection.reject().unwrap();
                return;
            }

            // Get the client info
            let client = connection.use_protocol("rust-websocket").accept().unwrap();

            let ip = client.peer_addr().unwrap();
            println!("New client connection: {:?}", ip);

            let (mut receiver, sender) = client.split().unwrap();

            // Create a chat client with a random username (can be changed later)
            {
                let mut clients = thread_chat_clients.lock().unwrap();

                let chat_client = ChatClient {
                    ip,
                    username: create_random_username(),
                    sender,
                };

                clients.insert(ip, RwLock::new(chat_client));
            }

            // Listen to incoming messages
            for message in receiver.incoming_messages() {
                let message = message.unwrap();

                match message {
                    OwnedMessage::Close(_) => {
                        let mut clients = thread_chat_clients.lock().unwrap();
                        {
                            let chat_client = clients.get(&ip).unwrap();
                            let mut chat_client = chat_client.write().unwrap();
                            let sender = &mut chat_client.sender;

                            let message = OwnedMessage::Close(None);
                            sender.send_message(&message).unwrap();
                            println!("Client {} disconnected", ip);
                        }

                        clients.remove(&ip);
                        return;
                    }
                    OwnedMessage::Ping(ping) => {
                        let clients = thread_chat_clients.lock().unwrap();

                        let chat_client = clients.get(&ip).unwrap();
                        let mut chat_client = chat_client.write().unwrap();
                        let sender = &mut chat_client.sender;

                        let message = OwnedMessage::Pong(ping);
                        sender.send_message(&message).unwrap();
                    }
                    OwnedMessage::Text(text) => {
                        let clients = thread_chat_clients.lock().unwrap();

                        let chat_client = clients.get(&ip).unwrap();
                        let chat_client = chat_client.write().unwrap();

                        let data: ProtocolEvent = serde_json::from_str(&text).unwrap();

                        match data {
                            ProtocolEvent::MessageC2S(data) => {
                                // Create the message struct
                                let chat_message = ChatMessage {
                                    text: data.text,
                                    username: chat_client.username.to_owned(),
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis(),
                                };

                                // Force the lock tp be released so we can access all the clients later
                                drop(chat_client);

                                // Store chat message in database
                                insert_message_into_db(&thread_sqlite, &chat_message);

                                // Create message
                                let serialized = serde_json::to_string(&ProtocolEvent::MessageS2C(
                                    MessageS2CData {
                                        text: chat_message.text,
                                        username: chat_message.username,
                                        timestamp: chat_message.timestamp,
                                    },
                                ))
                                .unwrap();

                                let message = OwnedMessage::Text(serialized);

                                // Emit message to all other clients
                                for (_, client) in clients.iter() {
                                    let mut client = client.write().unwrap();
                                    let sender = &mut client.sender;

                                    sender.send_message(&message).unwrap();
                                }
                            }
                            ProtocolEvent::SetUsernameC2S(_data) => todo!(),
                            _ => todo!(),
                        }
                    }
                    _ => todo!(),
                }
            }
        });
    }
}

fn init_sqlite(conn: &SQLiteConnection) {
    let conn = conn.lock().unwrap();

    conn.execute(
        "
        CREATE TABLE IF NOT EXISTS chat_messages (text TEXT, username VARCHAR(16), timestamp INTEGER);
        ",
    )
    .unwrap();
}

fn insert_message_into_db(conn: &SQLiteConnection, message: &ChatMessage) {
    let conn = conn.lock().unwrap();

    let query = format!(
        "INSERT INTO chat_messages (text, username, timestamp) VALUES ('{}', '{}', '{}');",
        &message.text, &message.username, &message.timestamp
    );

    conn.execute(query).unwrap();
}

fn create_random_username() -> String {
    format!(
        "user{}",
        Alphanumeric
            .sample_string(&mut rand::thread_rng(), 4)
            .to_uppercase()
    )
}
