use std::fmt::Display;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use websocket::sync::stream::Splittable;
use websocket::sync::Reader;
use websocket::{ClientBuilder, OwnedMessage};

use chat_server::{ClientBoundPacket, ServerBoundPacket};

/**
 * Translates messages between the TCP socket and Web Socket
 */
fn handle_client(stream: TcpStream) {
    // Split tcp stream to be used on different threads
    let (mut tcp_reader, mut tcp_writer) = stream.split().unwrap();

    // Create a websocket client to interact with chat server
    let ws_client = ClientBuilder::new("ws://127.0.0.1:8000")
        .unwrap()
        .connect_insecure()
        .unwrap();

    // Split web socket client to be used on different threads
    let (mut ws_reader, mut ws_writer) = ws_client.split().unwrap();

    // Handle incoming packets from the arduino
    thread::spawn(move || {
        let mut buf_reader = BufReader::new(&mut tcp_reader);

        let mut buf = String::with_capacity(512);
        loop {
            let result = buf_reader.read_line(&mut buf);

            match result {
                Err(_) => {
                    println!("Error reading line!");
                    // Todo shutdown stream and ws client
                    return;
                }
                Ok(n) => {
                    println!("Received {} bytes", n);
                    println!("Data: \"{}\"", buf);

                    ws_writer
                        .send_message(&OwnedMessage::Text("".to_owned()))
                        .unwrap();
                }
            }
        }
    });

    // Handle incoming ws messages
    thread::spawn(move || {
        for message in ws_reader.incoming_messages() {
            let message = match message {
                Err(_) => return,
                Ok(msg) => msg,
            };

            match handle_ws_message(message, &mut tcp_writer) {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("{}", err);
                    tcp_writer.shutdown(Shutdown::Both).unwrap();
                    ws_reader.shutdown().unwrap();
                    return;
                }
            }
        }
    });
}

enum WsError {
    MalformedPacket,
    TcpWrite,
}

impl Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::MalformedPacket => f.write_str("Received malformed JSON packet"),
            WsError::TcpWrite => f.write_str("Error writing to TCP socket"),
        }
    }
}

fn handle_ws_message(message: OwnedMessage, tcp_writer: &mut TcpStream) -> Result<(), WsError> {
    match message {
        OwnedMessage::Text(text) => {
            let packet: ClientBoundPacket =
                serde_json::from_str(&text).map_err(|_| WsError::MalformedPacket)?;

            match packet {
                ClientBoundPacket::Message {
                    text,
                    username,
                    timestamp,
                } => {
                    tcp_writer.write(b"");
                }
                ClientBoundPacket::ClientJoin { username } => {}
                ClientBoundPacket::ClientLeave { username } => {}
                ClientBoundPacket::ClientTyping { username } => {}
            }

            // Echo ws messages to the arduino's tcp socket
            tcp_writer
                .write_all(text.as_bytes())
                .map_err(|_| WsError::TcpWrite)?;

            Ok(())
        }
        _ => Ok(()),
    }
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:3333").unwrap();
    // accept connections and process them, spawning a new thread for each one
    println!("Server listening on port 3333");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move || {
                    // connection succeeded
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
                /* connection failed */
            }
        }
    }
}
