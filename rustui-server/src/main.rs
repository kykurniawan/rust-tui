use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

type RoomClients = HashMap<String, mpsc::Sender<String>>;
type ClientMap = Arc<RwLock<HashMap<String, RoomClients>>>;

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Cmd {
    Auth { username: String, password: String },
    Broadcast { msg: String },
    List,
}

fn init_db() -> Arc<Mutex<rusqlite::Connection>> {
    let home = dirs::home_dir().expect("Could not determine home directory");
    let data_dir = home.join(".rustui");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
    let db_path = data_dir.join("rustui.db");
    let conn = rusqlite::Connection::open(db_path).expect("Failed to open database");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rooms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS user_rooms (
            user_id INTEGER NOT NULL,
            room_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, room_id),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE CASCADE
        );",
    )
    .expect("Failed to create tables");

    Arc::new(Mutex::new(conn))
}

fn check_user_room_access(
    db: &Mutex<rusqlite::Connection>,
    username: &str,
    room_name: &str,
) -> bool {
    let conn = db.lock().unwrap();
    conn.query_row(
        "SELECT 1 FROM users u
         JOIN user_rooms ur ON u.id = ur.user_id
         JOIN rooms r ON r.id = ur.room_id
         WHERE u.username = ?1 AND r.name = ?2",
        rusqlite::params![username, room_name],
        |_| Ok(()),
    )
    .is_ok()
}

fn validate_user(
    db: &Mutex<rusqlite::Connection>,
    username: &str,
    password: &str,
) -> bool {
    let conn = db.lock().unwrap();
    let password_hash = hash_password(password);
    conn.query_row(
        "SELECT 1 FROM users WHERE username = ?1 AND password_hash = ?2",
        rusqlite::params![username, password_hash],
        |_| Ok(()),
    )
    .is_ok()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = init_db();
    let clients: ClientMap = Arc::new(RwLock::new(HashMap::new()));

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("WebSocket server listening on ws://0.0.0.0:8080");
    println!("Connect to rooms at: ws://0.0.0.0:8080/room/<room-name>");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                println!("New connection from: {}", peer_addr);
                let clients = clients.clone();
                let db = db.clone();

                tokio::spawn(async move {
                    let mut room_name = String::new();

                    let ws_stream = match tokio_tungstenite::accept_hdr_async(
                        stream,
                        |req: &Request, response: Response| {
                            let path = req.uri().path().to_string();
                            if let Some(name) = path.strip_prefix("/room/") {
                                if !name.is_empty() {
                                    room_name = name.to_string();
                                }
                            }
                            Ok(response)
                        },
                    )
                    .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            println!("WebSocket handshake error: {}", e);
                            return;
                        }
                    };

                    // Validate room exists
                    {
                        let conn = db.lock().unwrap();
                        let exists: bool = conn
                            .query_row(
                                "SELECT 1 FROM rooms WHERE name = ?1",
                                [&room_name],
                                |_| Ok(()),
                            )
                            .is_ok();
                        if !exists {
                            println!(
                                "Rejected {}: room '{}' not found",
                                peer_addr, room_name
                            );
                            return;
                        }
                    }

                    // Ensure room entry exists in the client map
                    {
                        let mut clients = clients.write().await;
                        clients.entry(room_name.clone()).or_default();
                    }

                    let (mut write, mut read) = ws_stream.split();
                    let mut my_username = String::new();
                    let peer_str = peer_addr.to_string();

                    let (tx, mut rx) = mpsc::channel::<String>(100);

                    {
                        let mut clients = clients.write().await;
                        if let Some(room) = clients.get_mut(&room_name) {
                            room.insert(peer_str.clone(), tx);
                        }
                    }

                    let mut heartbeat = interval(std::time::Duration::from_secs(30));

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
                                                    if validate_user(&db, &username, &password) {
                                                        if !check_user_room_access(&db, &username, &room_name) {
                                                            write.send(Message::Text(
                                                                serde_json::json!({"type": "error", "msg": "Access denied to this room"}).to_string()
                                                            )).await.ok();
                                                            continue;
                                                        }

                                                        let (new_tx, new_rx) = mpsc::channel::<String>(100);

                                                        {
                                                            let mut clients = clients.write().await;
                                                            if let Some(room) = clients.get_mut(&room_name) {
                                                                room.remove(&peer_str);
                                                                if let Some(old_tx) = room.remove(&username) {
                                                                    drop(old_tx);
                                                                }
                                                                my_username = username.clone();
                                                                room.insert(username.clone(), new_tx);
                                                            }
                                                        }

                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "authenticated", "username": username, "room": room_name}).to_string()
                                                        )).await.ok();

                                                        println!("User '{}' authenticated in room '{}'", username, room_name);

                                                        let join_msg = serde_json::json!({
                                                            "type": "system",
                                                            "msg": format!("{} joined the chat", username)
                                                        }).to_string();

                                                        let ids: Vec<String>;
                                                        {
                                                            let clients = clients.read().await;
                                                            if let Some(room) = clients.get(&room_name) {
                                                                ids = room.keys().cloned().collect();
                                                                let list_msg = serde_json::json!({"type": "list", "clients": ids.clone()}).to_string();

                                                                for (client_id, sender) in room.iter() {
                                                                    if client_id != &username {
                                                                        let _ = sender.send(join_msg.clone()).await;
                                                                        let _ = sender.send(list_msg.clone()).await;
                                                                    }
                                                                }
                                                            } else {
                                                                ids = vec![];
                                                            }
                                                        }

                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "list", "clients": ids}).to_string()
                                                        )).await.ok();

                                                        rx = new_rx;
                                                    } else {
                                                        write.send(Message::Text(
                                                            serde_json::json!({"type": "error", "msg": "Invalid credentials"}).to_string()
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
                                                        if let Some(room) = clients.get(&room_name) {
                                                            let payload = serde_json::json!({
                                                                "type": "message",
                                                                "from": my_username,
                                                                "msg": msg
                                                            }).to_string();
                                                            for (id, sender) in room.iter() {
                                                                if id != &my_username {
                                                                    let _ = sender.send(payload.clone()).await;
                                                                }
                                                            }
                                                            println!("Broadcast from {} in room {}: {}", my_username, room_name, msg);
                                                        }
                                                    }
                                                }
                                                Cmd::List => {
                                                    if !my_username.is_empty() {
                                                        let clients = clients.read().await;
                                                        if let Some(room) = clients.get(&room_name) {
                                                            let ids: Vec<String> = room.keys().cloned().collect();
                                                            write.send(Message::Text(
                                                                serde_json::json!({"type": "list", "clients": ids}).to_string()
                                                            )).await.ok();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        println!("Client {} disconnected from room {}", peer_addr, room_name);
                                        break;
                                    }
                                    None | Some(Err(_)) => {
                                        println!("Client {} connection error in room {}", peer_addr, room_name);
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ = heartbeat.tick() => {
                                if write.send(Message::Ping(vec![])).await.is_err() {
                                    println!("Client {} heartbeat failed, disconnecting from room {}", peer_addr, room_name);
                                    break;
                                }
                            }
                        }
                    }

                    // Always clean up on loop exit
                    cleanup_client(&clients, &room_name, &my_username, &peer_str).await;
                });
            }
            Err(e) => {
                println!("Error accepting connection: {}", e);
            }
        }
    }
}

async fn cleanup_client(
    clients: &ClientMap,
    room_name: &str,
    username: &str,
    peer_str: &str,
) {
    let mut clients = clients.write().await;
    if let Some(room) = clients.get_mut(room_name) {
        if !username.is_empty() {
            room.remove(username);

            let leave_msg = serde_json::json!({
                "type": "system",
                "msg": format!("{} left the chat", username)
            })
            .to_string();

            let ids: Vec<String> = room.keys().cloned().collect();
            let list_msg = serde_json::json!({"type": "list", "clients": ids}).to_string();
            for (_, sender) in room.iter() {
                let _ = sender.send(leave_msg.clone()).await;
                let _ = sender.send(list_msg.clone()).await;
            }
        } else {
            room.remove(peer_str);
        }
    }
}
