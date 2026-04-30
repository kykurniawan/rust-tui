# RustUI Chat Application

A terminal-based chat application built with Rust, featuring a WebSocket server and TUI client.

## Workspace Structure

This project uses Cargo workspace with resolver version 3:

- **rustui-client**: Terminal UI chat client
- **rustui-server**: WebSocket chat server

## Building

Build all workspace members:
```bash
cargo build
```

Build specific crate:
```bash
cargo build -p rustui-client
cargo build -p rustui-server
```

## Running

Start the server:
```bash
cargo run -p rustui-server
```

In another terminal, start the client:
```bash
cargo run -p rustui-client
```

## Features

- WebSocket-based real-time chat
- Terminal UI with crossterm and tui-rs
- User authentication
- Multi-user support
- Message broadcasting
- **Smart message scrolling** with auto-scroll
- **Multi-line message wrapping** - long messages wrap properly
- Participant list tracking

## Default Users

- admin / secret123
- rizky / pass123
- john / john123

## Controls

### Login Screen
- **TAB**: Switch between username and password fields
- **ENTER**: Submit login
- **BACKSPACE**: Delete character

### Chat Screen

**Focus Management:**
- **SHIFT+TAB**: Toggle focus between Message List and Input Field
  - Focused section is highlighted with `[FOCUSED]` indicator
  - Message List: Cyan border when focused
  - Input Field: Green border when focused

**When Message List is Focused:**
- **UP/DOWN**: Scroll through messages one at a time
- **PAGE UP/PAGE DOWN**: Scroll through messages 10 at a time
- **HOME**: Jump to first message
- **END**: Jump to the latest message (bottom)

**When Input Field is Focused:**
- **Type**: Enter text
- **LEFT/RIGHT**: Move cursor horizontally
- **UP/DOWN**: Move cursor vertically (for multi-line input)
- **HOME**: Move cursor to start of input
- **END**: Move cursor to end of input
- **BACKSPACE**: Delete character before cursor
- **DELETE**: Delete character after cursor
- **ENTER**: Send message

**Global:**
- **ESC**: Exit application
