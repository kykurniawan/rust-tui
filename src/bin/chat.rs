use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng;
use rustui::{App, Spans, draw_chat_screen, get_timestamp};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

fn generate_id() -> String {
    let mut rng = rand::thread_rng();
    (0..6).map(|_| {
        let idx = rng.gen_range(0..36);
        if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
    }).collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let my_id = generate_id();
    println!("Your ID: {}", my_id);

    let url = "ws://127.0.0.1:8080";
    let (ws_stream, _) = connect_async(url).await?;
    println!("Connected to server: {}", url);

    let (mut write, mut read) = ws_stream.split();

    let register_msg = serde_json::json!({
        "Register": { "id": &my_id }
    });
    write.send(Message::Text(register_msg.to_string())).await?;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    let mut app = App::new(my_id.clone());

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let _ = tx.send(text).await;
            }
        }
    });

    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_chat_screen(f, size, &mut app);
        })?;

        if let Some(msg) = rx.try_recv().ok() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "message" => {
                        let from = json.get("from").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let text = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        let ts = get_timestamp();
                        app.add_message(format!(
                            "[{}] {}: {}",
                            ts,
                            from,
                            text
                        ));
                    }
                    "list" => {
                        if let Some(clients) = json.get("clients").and_then(|v| v.as_array()) {
                            let ids: Vec<String> = clients.iter()
                                .filter_map(|c| c.as_str().map(String::from))
                                .collect();
                            app.set_participants(ids);
                        }
                    }
                    "registered" => {
                        if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                            app.set_participants(vec![id.to_string()]);
                        }
                    }
                    "error" => {
                        let err_msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        app.add_message(format!("[system] Error: {}", err_msg));
                    }
                    _ => {}
                }
            } else {
                app.add_message(msg);
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
                            if !app.input.is_empty() {
                                let input = app.input.trim().to_string();
                                
                                let ts = get_timestamp();
                                let msg = format!("[{}] {}: {}", ts, app.my_id, input);
                                app.messages.push(Spans::from(msg));
                                
                                let send_msg = serde_json::json!({
                                    "Broadcast": { "msg": input }
                                });
                                let _ = write.send(Message::Text(send_msg.to_string())).await;
                                
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