// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The connection library the manager shows.
//!
//! `Protocol` names a concrete client implementation, not just a wire
//! protocol: a VNC server can be reached through the built-in Rust client,
//! LibVNC or TigerVNC, and an RDP server through FreeRDP or IronRDP.

use serde::{Deserialize, Serialize};

use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// The built-in, pure-Rust RFB client.
    #[serde(rename = "vnc", alias = "vnc-rust", alias = "rfb")]
    VncRust,
    /// libvncclient, through the vendored C library.
    #[serde(rename = "libvnc")]
    VncLibVnc,
    /// TigerVNC's viewer core, through the vendored C library.
    #[serde(rename = "tigervnc")]
    VncTigerVnc,
    /// FreeRDP, through the vendored C library.
    #[serde(rename = "rdp", alias = "freerdp")]
    RdpFreeRdp,
    /// IronRDP, the pure-Rust RDP implementation.
    #[serde(rename = "ironrdp")]
    RdpIronRdp,
    /// SPICE, through ryll.
    #[serde(rename = "spice")]
    Spice,
    /// RustDesk's protocol.
    #[serde(rename = "rustdesk")]
    RustDesk,
}

impl Protocol {
    /// The VNC client implementations, which are wired up.
    pub const VNC_IMPLEMENTATIONS: [Protocol; 3] = [
        Protocol::VncTigerVnc,
        Protocol::VncLibVnc,
        Protocol::VncRust,
    ];

    /// The RDP client implementations.
    pub const RDP_IMPLEMENTATIONS: [Protocol; 2] = [Protocol::RdpFreeRdp, Protocol::RdpIronRdp];

    /// Kept in the editor as saved-connection vocabulary. SPICE is wired up;
    /// RustDesk is not yet.
    pub const OTHER_PROTOCOLS: [Protocol; 2] = [Protocol::Spice, Protocol::RustDesk];

    pub fn label(self) -> &'static str {
        match self {
            Protocol::VncRust => "Rust VNC",
            Protocol::VncLibVnc => "LibVNC",
            Protocol::VncTigerVnc => "TigerVNC",
            Protocol::RdpFreeRdp => "FreeRDP",
            Protocol::RdpIronRdp => "IronRDP",
            Protocol::Spice => "SPICE",
            Protocol::RustDesk => "RustDesk",
        }
    }

    /// The protocol family, for a badge that should not get as long as the
    /// implementation name.
    pub fn family(self) -> &'static str {
        match self {
            Protocol::VncRust | Protocol::VncLibVnc | Protocol::VncTigerVnc => "VNC",
            Protocol::RdpFreeRdp | Protocol::RdpIronRdp => "RDP",
            Protocol::Spice => "SPICE",
            Protocol::RustDesk => "RustDesk",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Protocol::VncRust => "Built-in, no C dependencies",
            Protocol::VncLibVnc => "Lowest bandwidth, low CPU",
            Protocol::VncTigerVnc => "Fastest, H.264 capable",
            Protocol::RdpFreeRdp => "Most robust, H.264",
            Protocol::RdpIronRdp => "Lighter CPU, pure Rust",
            Protocol::Spice => "Virtual machines",
            Protocol::RustDesk => "Direct or relayed",
        }
    }

    pub fn color(self) -> gpui::Rgba {
        match self {
            Protocol::VncRust => theme::protocol_vnc(),
            Protocol::VncLibVnc => theme::protocol_libvnc(),
            Protocol::VncTigerVnc => theme::protocol_tigervnc(),
            Protocol::RdpFreeRdp => theme::protocol_rdp(),
            Protocol::RdpIronRdp => theme::protocol_ironrdp(),
            Protocol::Spice => theme::protocol_spice(),
            Protocol::RustDesk => theme::protocol_rustdesk(),
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Protocol::VncRust | Protocol::VncLibVnc | Protocol::VncTigerVnc => 5900,
            Protocol::RdpFreeRdp | Protocol::RdpIronRdp => 3389,
            Protocol::Spice => 5930,
            Protocol::RustDesk => 21118,
        }
    }
}

/// `host`, `host:port` or `[v6]:port`, with 5900 when no port is given.
pub fn parse_address(input: &str) -> Result<(String, u16), String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter an address".into());
    }
    let (host, port) = if let Some(rest) = input.strip_prefix('[') {
        let Some((host, after)) = rest.split_once(']') else {
            return Err("Unclosed IPv6 address".into());
        };
        if host.is_empty() {
            return Err("Enter a host".into());
        }
        match after.strip_prefix(':') {
            Some(port) => (
                host.to_string(),
                port.parse::<u16>()
                    .map_err(|_| format!("Invalid port: {port}"))?,
            ),
            None if after.is_empty() => (host.to_string(), 5900),
            _ => return Err("Invalid address".into()),
        }
    } else if let Some((host, port)) = input.rsplit_once(':') {
        if host.is_empty() {
            return Err("Enter a host".into());
        }
        (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|_| format!("Invalid port: {port}"))?,
        )
    } else {
        (input.to_string(), 5900)
    };
    if host.is_empty() {
        Err("Enter a host".into())
    } else {
        Ok((host, port))
    }
}

#[derive(Clone)]
pub struct Connection {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub protocol: Protocol,
    pub username: String,
    pub last_used: String,
    pub online: bool,
    pub send_clipboard: bool,
    pub receive_clipboard: bool,
    pub grab_keyboard: bool,
    pub save_password: bool,
}

impl Connection {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub fn sample_connections() -> Vec<Connection> {
    vec![
        Connection {
            name: "Studio Mac".into(),
            host: "192.168.1.20".into(),
            port: 5900,
            protocol: Protocol::VncTigerVnc,
            username: "alex".into(),
            last_used: "12 minutes ago".into(),
            online: true,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
            save_password: true,
        },
        Connection {
            name: "Mac mini".into(),
            host: "192.168.1.31".into(),
            port: 5900,
            protocol: Protocol::VncLibVnc,
            username: String::new(),
            last_used: "Yesterday".into(),
            online: true,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
            save_password: true,
        },
        Connection {
            name: "Windows Workstation".into(),
            host: "192.168.1.42".into(),
            port: 3389,
            protocol: Protocol::RdpFreeRdp,
            username: "alex".into(),
            last_used: "3 days ago".into(),
            online: true,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
            save_password: true,
        },
        Connection {
            name: "Proxmox — Build VM".into(),
            host: "192.168.1.5".into(),
            port: 5930,
            protocol: Protocol::Spice,
            username: "root".into(),
            last_used: "Last week".into(),
            online: false,
            send_clipboard: true,
            receive_clipboard: true,
            grab_keyboard: true,
            save_password: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::parse_address;

    #[test]
    fn a_bare_host_uses_the_vnc_port() {
        assert_eq!(
            parse_address("192.0.2.10").unwrap(),
            ("192.0.2.10".to_string(), 5900)
        );
        assert_eq!(
            parse_address("  mac-mini.local  ").unwrap(),
            ("mac-mini.local".to_string(), 5900)
        );
    }

    #[test]
    fn an_explicit_port_wins() {
        assert_eq!(
            parse_address("192.0.2.10:5901").unwrap(),
            ("192.0.2.10".to_string(), 5901)
        );
    }

    #[test]
    fn brackets_carry_an_ipv6_address() {
        assert_eq!(
            parse_address("[2001:db8::1]:5900").unwrap(),
            ("2001:db8::1".to_string(), 5900)
        );
        assert_eq!(
            parse_address("[2001:db8::1]").unwrap(),
            ("2001:db8::1".to_string(), 5900)
        );
    }

    #[test]
    fn empty_and_invalid_inputs_are_rejected() {
        assert!(parse_address("").is_err());
        assert!(parse_address("   ").is_err());
        assert!(parse_address("host:not-a-port").is_err());
        assert!(parse_address("host:99999").is_err());
        assert!(parse_address("[::1").is_err());
    }
}
