use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rustui::{App, Spans, draw_chat_screen, get_timestamp};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

fn read_line() -> String {
    let mut input = String::new();
    loop {
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Event::Key(key) = event::read().unwrap() {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => {
                            input.push(c);
                            print!("{}", c);
                        }
                        KeyCode::Backspace => {
                            input.pop();
                            print!("\x08 \x08");
                        }
                        KeyCode::Enter => {
                            println!();
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    input
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;

    println!("=== SECURE CHAT ===");
    print!("Username: ");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let username = read_line();
    
    print!("Password: ");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let password = read_line();

    let rt = tokio::runtime::Runtime::new()?;

    let url = "ws://127.0.0.1:8080";
    let (ws_stream, _) = rt.block_on(async { connect_async(url).await })?;
    println!("Connected to server!");

    let (mut write, mut read) = ws_stream.split();

    let auth_msg = serde_json::json!({
        "Auth": { "username": &username, "password": &password }
    });
    rt.block_on(async { write.send(Message::Text(auth_msg.to_string())).await })?;

    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    let mut app = App::new();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    rt.spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let _ = tx.send(text).await;
            }
        }
    });

    let mut authenticated = false;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_chat_screen(f, size, &mut app);
        })?;

        if let Some(msg) = rx.try_recv().ok() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "authenticated" => {
                        if let Some(user) = json.get("username").and_then(|v| v.as_str()) {
                            app.init(user.to_string());
                            authenticated = true;
                        }
                    }
                    "message" => {
                        let from = json.get("from").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let text = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        let ts = get_timestamp();
                        app.add_message(format!("[{}] {}: {}", ts, from, text));
                    }
                    "list" => {
                        if let Some(clients) = json.get("clients").and_then(|v| v.as_array()) {
                            let ids: Vec<String> = clients.iter()
                                .filter_map(|c| c.as_str().map(String::from))
                                .collect();
                            app.set_participants(ids);
                        }
                    }
                    "error" => {
                        let err_msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        app.add_message(format!("[system] Error: {}", err_msg));
                    }
                    _ => {}
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Enter => {
                            if !app.input.is_empty() && authenticated {
                                let input = app.input.trim().to_string();
                                let ts = get_timestamp();
                                let msg = format!("[{}] {}: {}", ts, username, input);
                                app.messages.push(Spans::from(msg));
                                
                                let send_msg = serde_json::json!({
                                    "Broadcast": { "msg": input }
                                });
                                let _ = rt.block_on(async { write.send(Message::Text(send_msg.to_string())).await });
                                
                                app.input.clear();
                            }
                        }
                        KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}