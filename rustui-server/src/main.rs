use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

type ClientMap = Arc<RwLock<HashMap<String, mpsc::Sender<String>>>>;

#[derive(serde::Serialize, serde::Deserialize)]
enum Cmd {
    Auth { username: String, password: String },
    Broadcast { msg: String },
    List,
}

fn load_users() -> HashMap<String, String> {
    let mut users = HashMap::new();
    users.insert("admin".to_string(), "secret123".to_string());
    users.insert("rizky".to_string(), "pass123".to_string());
    users.insert("john".to_string(), "john123".to_string());
    users
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clients: ClientMap = Arc::new(RwLock::new(HashMap::new()));
    let users = Arc::new(load_users());

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("WebSocket server listening on ws://127.0.0.1:8080");
    println!("Available users:");
    for (username, _) in users.iter() {
        println!("  - {}", username);
    }

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                println!("New connection from: {}", peer_addr);
                let clients = clients.clone();
                let users = users.clone();

                tokio::spawn(async move {
                    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Error accepting connection: {}", e);
                            return;
                        }
                    };

                    let (mut write, mut read) = ws_stream.split();
                    let mut my_username = String::new();
                    let peer_str = peer_addr.to_string();

                    let (tx, mut rx) = mpsc::channel::<String>(100);

                    {
                        let mut clients = clients.write().await;
                        clients.insert(peer_str.clone(), tx);
                    }

                    loop {
                        tokio::select! {
                            msg = rx.recv() => {
                                if let Some(text) = msg {
                                    if write.send(Message::Text(text)).await.is_err() {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Ok(cmd) = serde_json::from_str::<Cmd>(&text) {
                                            match cmd {
                                                Cmd::Auth { username, password } => {
                                                    if let Some(stored_pass) = users.get(&username) {
                                                        if *stored_pass == password {
                                                            let (new_tx, new_rx) = mpsc::channel::<String>(100);
                                                            
                                                            {
                                                                let mut clients = clients.write().await;
                                                                clients.remove(&peer_str);
                                                                if let Some(old_tx) = clients.remove(&username) {
                                                                    drop(old_tx);
                                                                }
                                                                my_username = username.clone();
                                                                clients.insert(username.clone(), new_tx);
                                                            }

                                                            // Send authenticated message first
                                                            write.send(Message::Text(
                                                                serde_json::json!({"type": "authenticated", "username": username}).to_string()
                                                            )).await.ok();
                                                            
                                                            println!("User authenticated: {}", username);

                                                            // Broadcast "user joined" system message to all other clients
                                                            let join_msg = serde_json::json!({
                                                                "type": "system",
                                                                "msg": format!("{} joined the chat", username)
                                                            }).to_string();

                                                            // Get the updated client list and broadcast to everyone
                                                            let ids: Vec<String>;
                                                            {
                                                                let clients = clients.read().await;
                                                                ids = clients.keys().cloned().collect();
                                                                let list_msg = serde_json::json!({"type": "list", "clients": ids.clone()}).to_string();
                                                                
                                                                // Send to all OTHER clients via their channels
                                                                for (client_id, sender) in clients.iter() {
                                                                    if client_id != &username {
                                                                        let _ = sender.send(join_msg.clone()).await;
                                                                        let _ = sender.send(list_msg.clone()).await;
                                                                    }
                                                                }
                                                            }
                                                            
                                                            // Send directly to THIS client via WebSocket (not channel)
                                                            write.send(Message::Text(
                                                                serde_json::json!({"type": "list", "clients": ids}).to_string()
                                                            )).await.ok();
                                                            
                                                            // Switch to the new receiver AFTER sending the list
                                                            rx = new_rx;
                                                        } else {
                                                            write.send(Message::Text(
                                                                serde_json::json!({"type": "error", "msg": "Invalid credentials"}).to_string()
                                                            )).await.ok();
                                                        }
                                                    } else {
                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "error", "msg": "User not found"}).to_string()
                                                        )).await.ok();
                                                    }
                                                }
                                                Cmd::Broadcast { msg } => {
                                                    if my_username.is_empty() {
                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "error", "msg": "Not authenticated"}).to_string()
                                                        )).await.ok();
                                                    } else {
                                                        let clients = clients.read().await;
                                                        let payload = serde_json::json!({
                                                            "type": "message",
                                                            "from": my_username,
                                                            "msg": msg
                                                        }).to_string();
                                                        for (id, sender) in clients.iter() {
                                                            if id != &my_username {
                                                                let _ = sender.send(payload.clone()).await;
                                                            }
                                                        }
                                                        println!("Broadcast from {}: {}", my_username, msg);
                                                    }
                                                }
                                                Cmd::List => {
                                                    if !my_username.is_empty() {
                                                        let clients = clients.read().await;
                                                        let ids: Vec<String> = clients.keys().cloned().collect();
                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "list", "clients": ids}).to_string()
                                                        )).await.ok();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        println!("Client {} disconnected", peer_addr);
                                        if !my_username.is_empty() {
                                            let mut clients = clients.write().await;
                                            let leaving_user = my_username.clone();
                                            clients.remove(&my_username);
                                            
                                            // Broadcast "user left" system message
                                            let leave_msg = serde_json::json!({
                                                "type": "system",
                                                "msg": format!("{} left the chat", leaving_user)
                                            }).to_string();
                                            
                                            let ids: Vec<String> = clients.keys().cloned().collect();
                                            let list_msg = serde_json::json!({"type": "list", "clients": ids}).to_string();
                                            for (_, sender) in clients.iter() {
                                                let _ = sender.send(leave_msg.clone()).await;
                                                let _ = sender.send(list_msg.clone()).await;
                                            }
                                        } else {
                                            let mut clients = clients.write().await;
                                            clients.remove(&peer_str);
                                        }
                                        break;
                                    }
                                    None | Some(Err(_)) => {
                                        println!("Client {} connection error or closed", peer_addr);
                                        if !my_username.is_empty() {
                                            let mut clients = clients.write().await;
                                            let leaving_user = my_username.clone();
                                            clients.remove(&my_username);
                                            
                                            // Broadcast "user left" system message
                                            let leave_msg = serde_json::json!({
                                                "type": "system",
                                                "msg": format!("{} left the chat", leaving_user)
                                            }).to_string();
                                            
                                            let ids: Vec<String> = clients.keys().cloned().collect();
                                            let list_msg = serde_json::json!({"type": "list", "clients": ids}).to_string();
                                            for (_, sender) in clients.iter() {
                                                let _ = sender.send(leave_msg.clone()).await;
                                                let _ = sender.send(list_msg.clone()).await;
                                            }
                                        } else {
                                            let mut clients = clients.write().await;
                                            clients.remove(&peer_str);
                                        }
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                });
            }
            Err(e) => {
                println!("Error accepting connection: {}", e);
            }
        }
    }
}
