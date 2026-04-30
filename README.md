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

## Default Users

- admin / secret123
- rizky / pass123
- john / john123

## Controls

- **TAB**: Switch between username and password fields (login screen)
- **ENTER**: Submit login / Send message
- **ESC**: Exit application
- **UP/DOWN**: Navigate messages
- **BACKSPACE**: Delete character
