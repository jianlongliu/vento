// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The saved connection library, kept in the same XDG data directory the
//! existing Linux client uses for its extra libraries:
//! `$XDG_DATA_HOME/vento/connections.json` (or `~/.local/share/vento`).

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{Connection, Protocol};

#[derive(Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    connections: Vec<StoredConnection>,
}

#[derive(Serialize, Deserialize)]
struct StoredConnection {
    name: String,
    host: String,
    port: u16,
    protocol: Protocol,
    #[serde(default)]
    username: String,
    #[serde(default = "yes")]
    send_clipboard: bool,
    #[serde(default = "yes")]
    receive_clipboard: bool,
    #[serde(default = "yes")]
    grab_keyboard: bool,
    #[serde(default = "yes")]
    save_password: bool,
}

fn yes() -> bool {
    true
}

impl From<&Connection> for StoredConnection {
    fn from(c: &Connection) -> Self {
        Self {
            name: c.name.clone(),
            host: c.host.clone(),
            port: c.port,
            protocol: c.protocol,
            username: c.username.clone(),
            send_clipboard: c.send_clipboard,
            receive_clipboard: c.receive_clipboard,
            grab_keyboard: c.grab_keyboard,
            save_password: c.save_password,
        }
    }
}

impl From<StoredConnection> for Connection {
    fn from(c: StoredConnection) -> Self {
        Self {
            name: c.name,
            host: c.host,
            port: c.port,
            protocol: c.protocol,
            username: c.username,
            last_used: String::new(),
            online: false,
            send_clipboard: c.send_clipboard,
            receive_clipboard: c.receive_clipboard,
            grab_keyboard: c.grab_keyboard,
            save_password: c.save_password,
        }
    }
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/share")
        });
    base.join("vento")
}

pub fn path() -> PathBuf {
    data_dir().join("connections.json")
}

/// The session state directory, matching the existing Linux client's
/// `$XDG_STATE_HOME/vento` default.
pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/state")
        });
    base.join("vento")
}

fn encode(connections: &[Connection]) -> io::Result<Vec<u8>> {
    let store = StoreFile {
        version: 1,
        connections: connections.iter().map(StoredConnection::from).collect(),
    };
    serde_json::to_vec_pretty(&store).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn decode(data: &[u8]) -> io::Result<Vec<Connection>> {
    let store: StoreFile =
        serde_json::from_slice(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(store
        .connections
        .into_iter()
        .map(Connection::from)
        .collect())
}

pub fn load() -> io::Result<Vec<Connection>> {
    let path = path();
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    decode(&data)
}

pub fn save(connections: &[Connection]) -> io::Result<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let data = encode(connections)?;
    let path = path();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Protocol;

    fn connection(protocol: Protocol) -> Connection {
        Connection {
            name: "Machine".into(),
            host: "192.0.2.10".into(),
            port: protocol.default_port(),
            protocol,
            username: "user".into(),
            last_used: "yesterday".into(),
            online: false,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
            save_password: true,
        }
    }

    #[test]
    fn every_backend_survives_a_round_trip() {
        let connections = vec![
            connection(Protocol::VncTigerVnc),
            connection(Protocol::VncLibVnc),
            connection(Protocol::VncRust),
            connection(Protocol::RdpFreeRdp),
            connection(Protocol::RdpIronRdp),
            connection(Protocol::Spice),
            connection(Protocol::RustDesk),
        ];
        let decoded = decode(&encode(&connections).unwrap()).unwrap();
        assert_eq!(decoded.len(), connections.len());
        for (a, b) in connections.iter().zip(decoded.iter()) {
            assert_eq!(a.protocol, b.protocol);
            assert_eq!(a.name, b.name);
            assert_eq!(a.host, b.host);
            assert_eq!(a.port, b.port);
            assert_eq!(a.username, b.username);
        }
    }

    #[test]
    fn the_old_vnc_name_still_loads_as_the_rust_client() {
        let data = br#"{"version":1,"connections":[
            {"name":"Old","host":"example.test","port":5900,"protocol":"vnc","username":""}
        ]}"#;
        let decoded = decode(data).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].protocol, Protocol::VncRust);
    }

    #[test]
    fn a_broken_file_is_an_error_not_an_empty_library() {
        assert!(decode(b"not json").is_err());
    }
}
