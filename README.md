# IRCd-rs

A modern, async IRC server implementation in Rust following the TS6 protocol specification.

## Features

- Full TS6 protocol support for server linking
- Async I/O using Tokio
- Channel modes and user modes
- Server operator (IRCop) support
- Connection classes and access controls (K/G/D-lines)
- SSL/TLS support
- Database persistence for configuration
- Configurable via TOML

## Configuration

Create a `config.toml` file:

```
toml
[server]
name = "irc.example.com"
description = "My IRC Server"
sid = "001" # Server ID for TS6
bind_addr = "0.0.0.0"
port = 6667
[network]
name = "ExampleNet"
[limits]
max_clients = 1000
max_channels = 100
[access]
K-lines (bans)
klines = []
G-lines (global bans)
glines = []
I-lines (connection classes)
ilines = []
O-lines (IRC operators)
olines = []
Optional server links
[[links]]
name = "hub.example.com"
sid = "002"
description = "Hub Server"
password = "linkpass"
address = "hub.example.com:6667"
autoconnect = true
ssl = false
```

## Building

```
cargo build --release
```

## Running

```
./target/release/ircd-rs -c config.toml
```
