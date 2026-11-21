// SPDX-License-Identifier: GPL-3.0-or-later
//
//! What the About sheet and the command-line help report: the app, the
//! upstream backend sources, the GUI toolkit, the compiler, and every
//! backend library that can be loaded right now.

use crate::model::Protocol;

pub fn rows() -> Vec<(&'static str, String)> {
    vec![
        ("Ventox", env!("CARGO_PKG_VERSION").to_string()),
        (
            "Vento backend sources",
            env!("VENTO_BACKEND_VERSION").to_string(),
        ),
        ("GPUI", env!("GPUI_VERSION").to_string()),
        ("Rust", env!("VENTOX_RUSTC_VERSION").to_string()),
        ("TigerVNC", (remotedesktop_tigervnc::KIND.version)()),
        ("LibVNC", (remotedesktop_libvnc::KIND.version)()),
        ("Rust VNC", (remotedesktop_rfb::backend::KIND.version)()),
        ("FreeRDP", (remotedesktop_freerdp::KIND.version)()),
        ("IronRDP", (remotedesktop_ironrdp::backend::KIND.version)()),
        ("SPICE", (remotedesktop_spice::backend::KIND.version)()),
        ("RustDesk", "not wired up yet".to_string()),
    ]
}

pub fn backend_rows() -> Vec<(String, String)> {
    Protocol::VNC_IMPLEMENTATIONS
        .into_iter()
        .chain(Protocol::RDP_IMPLEMENTATIONS)
        .chain(Protocol::OTHER_PROTOCOLS)
        .map(|protocol| {
            let version = match protocol {
                Protocol::VncTigerVnc => (remotedesktop_tigervnc::KIND.version)(),
                Protocol::VncLibVnc => (remotedesktop_libvnc::KIND.version)(),
                Protocol::VncRust => (remotedesktop_rfb::backend::KIND.version)(),
                Protocol::RdpFreeRdp => (remotedesktop_freerdp::KIND.version)(),
                Protocol::RdpIronRdp => (remotedesktop_ironrdp::backend::KIND.version)(),
                Protocol::Spice => (remotedesktop_spice::backend::KIND.version)(),
                Protocol::RustDesk => "not wired up yet".to_string(),
            };
            (
                format!("{} ({})", protocol.label(), protocol.family()),
                version,
            )
        })
        .collect()
}

pub fn print_help() {
    println!("Ventox {}", env!("CARGO_PKG_VERSION"));
    for (label, value) in rows() {
        println!("  {label:<22} {value}");
    }
    println!();
    println!("usage:");
    println!("  ventox                                connection manager");
    println!("  ventox --connect ADDRESS              connect directly (default TigerVNC)");
    println!("  ventox --backend NAME --connect ADDR  choose a backend");
    println!("  ventox --list-backends                backend versions");
    println!("  ventox --versions                     version summary");
    println!();
    println!("backends: tigervnc, libvnc, rfb, freerdp, ironrdp, spice, rustdesk");
    println!("environment: VENTOX_PASSWORD (direct connection), VENTOX_DEMO=1 (sample library)");
}

pub fn print_versions() {
    println!("Ventox {}", env!("CARGO_PKG_VERSION"));
    for (label, value) in rows() {
        println!("{label:<22} {value}");
    }
}
