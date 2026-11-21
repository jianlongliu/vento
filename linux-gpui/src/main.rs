// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Ventox: a Wayland-native VNC/RDP/SPICE client on GPUI.
//!
//! With no arguments the connection manager opens; `--connect ADDRESS` opens
//! a session on that address directly.

mod about;
mod app;
mod keys;
mod model;
mod passwords;
mod secrets;
mod session;
mod settings;
mod store;
mod text_input;
mod theme;

use app::VentoxApp;
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions,
    prelude::*, px, size,
};
use model::{Connection, Protocol};

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let connect = args
        .windows(2)
        .find(|pair| pair[0] == "--connect")
        .map(|pair| pair[1].clone());
    let backend = args
        .windows(2)
        .find(|pair| pair[0] == "--backend")
        .map(|pair| pair[1].clone());

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        about::print_help();
        return;
    }
    if args
        .iter()
        .any(|arg| arg == "-V" || arg == "--version" || arg == "--versions")
    {
        about::print_versions();
        return;
    }
    if args.iter().any(|arg| arg == "--list-backends") {
        list_backends();
        return;
    }

    let settings = settings::load();
    theme::apply(theme::palette_for(settings.theme, None));

    Application::new().run(move |cx: &mut App| {
        passwords::init(cx);
        cx.bind_keys(text_input::key_bindings());
        if let Some(address) = connect {
            let connection = connection_from_address(&address, backend.as_deref());
            let password = std::env::var("VENTOX_PASSWORD").unwrap_or_default();
            if let Err(e) = session::open(connection, password, true, cx) {
                log::error!("could not start the session: {e}");
                open_manager(cx);
            }
        } else {
            open_manager(cx);
        }
        cx.activate(true);
    });
}

fn list_backends() {
    for (protocol, version) in about::backend_rows() {
        println!("{protocol:<22} {version}");
    }
}

fn open_manager(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.), px(520.))),
            window_decorations: Some(WindowDecorations::Client),
            app_id: Some("dev.ventox.Ventox".into()),
            titlebar: Some(TitlebarOptions {
                title: Some("Ventox".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let app = cx.new(|cx| VentoxApp::new(cx));
            #[cfg(debug_assertions)]
            if std::env::var_os("VENTOX_PREVIEW_EDITOR").is_some() {
                if let Some(handle) = app.read(cx).preview_editor_handle(cx) {
                    window.focus(&handle);
                }
            }
            app
        },
    )
    .expect("failed to open the connection manager");
}

fn connection_from_address(address: &str, backend: Option<&str>) -> Connection {
    let protocol = match backend.unwrap_or("tigervnc") {
        "libvnc" => Protocol::VncLibVnc,
        "rfb" | "rust" => Protocol::VncRust,
        "freerdp" | "rdp" => Protocol::RdpFreeRdp,
        "ironrdp" => Protocol::RdpIronRdp,
        "spice" => Protocol::Spice,
        "rustdesk" => Protocol::RustDesk,
        _ => Protocol::VncTigerVnc,
    };
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (address.to_string(), 5900),
        },
        None => (address.to_string(), 5900),
    };
    Connection {
        name: address.to_string(),
        host,
        port,
        protocol,
        username: String::new(),
        last_used: "Just now".into(),
        online: false,
        send_clipboard: true,
        receive_clipboard: true,
        grab_keyboard: true,
        save_password: false,
    }
}
