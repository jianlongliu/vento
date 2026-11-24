// SPDX-License-Identifier: GPL-3.0-or-later
//
//! The connection manager: a library sidebar on the left and the saved
//! connections as preview cards on the right, with the editor as a sheet
//! over it.
//!
//! The visual language follows the sheet-and-card patterns of RealVNC
//! Connect and macOS: one strong title, quiet metadata, cards that lift on
//! hover, and a single accent colour reserved for the primary action and the
//! current selection.

use crate::about;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, SharedString,
    Window, div, px, rgba,
};

use crate::model::{Connection, Protocol};
use crate::passwords::Passwords;
use crate::settings::{self, Settings};
use crate::store;
use crate::text_input::TextInput;
use crate::theme;

pub struct VentoxApp {
    connections: Vec<Connection>,
    selected: Option<usize>,
    editor: Option<EditorState>,
    query_input: gpui::Entity<TextInput>,
    query: String,
    notice: Option<String>,
    context_menu: Option<ContextMenu>,
    quick_connect: Option<QuickConnect>,
    settings: Settings,
    settings_open: bool,
    persist: bool,
}

#[derive(Clone)]
struct QuickConnect {
    address: gpui::Entity<TextInput>,
    password: gpui::Entity<TextInput>,
    protocol: Protocol,
    notice: Option<String>,
}

#[derive(Clone, Copy)]
struct ContextMenu {
    index: usize,
    position: gpui::Point<gpui::Pixels>,
}

#[derive(Clone)]
struct EditorState {
    existing: Option<usize>,
    protocol: Protocol,
    name: gpui::Entity<TextInput>,
    host: gpui::Entity<TextInput>,
    port: gpui::Entity<TextInput>,
    username: gpui::Entity<TextInput>,
    password: gpui::Entity<TextInput>,
    send_clipboard: bool,
    receive_clipboard: bool,
    grab_keyboard: bool,
    save_password: bool,
}

impl VentoxApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let query_input = cx.new(|cx| TextInput::new("Search connections", "", cx));
        cx.observe(&query_input, |this, input, cx| {
            this.query = input.read(cx).content();
            cx.notify();
        })
        .detach();

        let demo = std::env::var_os("VENTOX_DEMO").is_some()
            || std::env::var_os("VENTOX_PREVIEW_EDITOR").is_some();
        let connections = if demo {
            crate::model::sample_connections()
        } else {
            match store::load() {
                Ok(connections) => connections,
                Err(e) => {
                    log::warn!("could not read {}: {e}", store::path().display());
                    Vec::new()
                }
            }
        };
        let mut app = Self {
            connections,
            selected: None,
            editor: None,
            query_input,
            query: String::new(),
            notice: None,
            context_menu: None,
            quick_connect: None,
            settings: crate::settings::load(),
            settings_open: false,
            persist: !demo,
        };
        if std::env::var_os("VENTOX_PREVIEW_QUICK").is_some() {
            let address = cx.new(|cx| TextInput::new("host or host:port", "192.168.1.31:5900", cx));
            let password =
                cx.new(|cx| TextInput::new("VNC password (optional)", "", cx).masked(true));
            app.quick_connect = Some(QuickConnect {
                address,
                password,
                protocol: Protocol::VncTigerVnc,
                notice: None,
            });
        }
        if std::env::var_os("VENTOX_PREVIEW_ABOUT").is_some()
            || std::env::var_os("VENTOX_PREVIEW_SETTINGS").is_some()
        {
            app.settings_open = true;
        }
        if std::env::var_os("VENTOX_PREVIEW_EDITOR").is_some() {
            app.editor = Some(app.make_editor(
                cx,
                Some(0),
                Protocol::VncTigerVnc,
                "Studio Mac",
                "192.168.1.20",
                5900,
                "alex",
                true,
                true,
                true,
                true,
            ));
        }
        app
    }

    fn make_editor(
        &self,
        cx: &mut Context<Self>,
        existing: Option<usize>,
        protocol: Protocol,
        name: &str,
        host: &str,
        port: u16,
        username: &str,
        send_clipboard: bool,
        receive_clipboard: bool,
        grab_keyboard: bool,
        save_password: bool,
    ) -> EditorState {
        let name: SharedString = name.to_owned().into();
        let host: SharedString = host.to_owned().into();
        let port: SharedString = port.to_string().into();
        let username: SharedString = username.to_owned().into();
        let address = format!("{host}:{port}");
        let cached_password: SharedString = cx
            .try_global::<Passwords>()
            .and_then(|passwords| passwords.get(&address))
            .or_else(|| {
                if save_password {
                    crate::secrets::load_password(&address)
                } else {
                    None
                }
            })
            .unwrap_or_default()
            .into();
        EditorState {
            existing,
            protocol,
            name: cx.new(move |cx| TextInput::new("Optional label", name, cx)),
            host: cx.new(move |cx| TextInput::new("hostname or IP", host, cx)),
            port: cx.new(move |cx| TextInput::new("5900", port, cx)),
            username: cx.new(move |cx| TextInput::new("Username", username, cx)),
            password: cx
                .new(move |cx| TextInput::new("Password", cached_password, cx).masked(true)),
            send_clipboard,
            receive_clipboard,
            grab_keyboard,
            save_password,
        }
    }

    fn editor_handles(&self, cx: &gpui::App) -> Vec<gpui::FocusHandle> {
        self.editor
            .as_ref()
            .map(|editor| {
                vec![
                    editor.name.read(cx).handle(),
                    editor.username.read(cx).handle(),
                    editor.host.read(cx).handle(),
                    editor.port.read(cx).handle(),
                    editor.password.read(cx).handle(),
                ]
            })
            .unwrap_or_default()
    }

    fn editor_password_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.editor
            .as_ref()
            .map(|editor| editor.password.read(cx).handle().is_focused(window))
            .unwrap_or(false)
    }

    fn editor_focus_next(&mut self, reverse: bool, window: &mut Window, cx: &mut Context<Self>) {
        let handles = self.editor_handles(cx);
        if handles.is_empty() {
            return;
        }
        let current = handles.iter().position(|handle| handle.is_focused(window));
        let next = match (current, reverse) {
            (Some(0), true) => handles.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % handles.len(),
            (None, _) => 0,
        };
        window.focus(&handles[next]);
        cx.notify();
    }

    #[cfg(debug_assertions)]
    pub fn preview_editor_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        self.editor
            .as_ref()
            .map(|editor| editor.name.read(cx).handle())
    }

    fn open_new_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.make_editor(
            cx,
            None,
            Protocol::VncTigerVnc,
            "",
            "",
            5900,
            "",
            self.settings.send_clipboard,
            self.settings.receive_clipboard,
            self.settings.grab_keyboard,
            true,
        );
        window.focus(&editor.name.read(cx).handle());
        self.editor = Some(editor);
        cx.notify();
    }

    fn open_editor(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.context_menu = None;
        let c = self.connections[index].clone();
        let editor = self.make_editor(
            cx,
            Some(index),
            c.protocol,
            &c.name,
            &c.host,
            c.port,
            &c.username,
            c.send_clipboard,
            c.receive_clipboard,
            c.grab_keyboard,
            c.save_password,
        );
        window.focus(&editor.name.read(cx).handle());
        self.editor = Some(editor);
        cx.notify();
    }

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        cx.notify();
    }

    /// Write the editor back into the library, returning the saved row.
    fn commit_editor(&mut self, cx: &mut Context<Self>) -> Option<Connection> {
        let editor = self.editor.clone()?;
        let name = editor.name.read(cx).content();
        let host = editor.host.read(cx).content();
        let port: u16 = editor.port.read(cx).content().parse().unwrap_or(5900);
        let username = editor.username.read(cx).content();
        let name = if name.trim().is_empty() {
            host.clone()
        } else {
            name
        };
        let connection = Connection {
            name,
            host,
            port,
            protocol: editor.protocol,
            username,
            last_used: "Just now".into(),
            online: false,
            send_clipboard: editor.send_clipboard,
            receive_clipboard: editor.receive_clipboard,
            grab_keyboard: editor.grab_keyboard,
            save_password: editor.save_password,
        };
        // Saving a connection keeps its password for this run, and, when
        // asked to, across runs in the encrypted secrets file.
        let password = editor.password.read(cx).content();
        if password.is_empty() {
            cx.global::<Passwords>().forget(&connection.address());
            crate::secrets::forget_password(&connection.address());
        } else {
            cx.global::<Passwords>()
                .set(&connection.address(), &password);
            if self.persist {
                if editor.save_password {
                    if let Err(e) = crate::secrets::store_password(&connection.address(), &password)
                    {
                        log::warn!("could not save the password: {e}");
                    }
                } else {
                    crate::secrets::forget_password(&connection.address());
                }
            }
        }
        let index = match editor.existing {
            Some(index) => {
                self.connections[index] = connection.clone();
                index
            }
            None => {
                self.connections.insert(0, connection.clone());
                0
            }
        };
        self.selected = Some(index);
        if self.persist
            && let Err(e) = store::save(&self.connections)
        {
            log::warn!("could not write {}: {e}", store::path().display());
        }
        Some(connection)
    }

    fn connect_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        self.context_menu = None;
        self.selected = Some(index);
        let connection = self.connections[index].clone();
        self.open_session_deferred(connection, String::new(), cx);
        cx.notify();
    }

    /// Open the session window after the current update finishes. Creating a
    /// second Wayland surface from inside an input callback (and therefore
    /// inside a frame) upsets blade on some drivers.
    fn open_session_deferred(
        &mut self,
        connection: Connection,
        password: String,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().downgrade();
        let persist = self.persist;
        cx.defer(move |cx| {
            if let Err(e) = crate::session::open(connection, password, persist, cx) {
                log::error!("could not start the session: {e}");
                this.update(cx, |this, cx| {
                    this.notice = Some(format!("Could not start the session: {e}"));
                    cx.notify();
                })
                .ok();
            }
        });
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = true;
        if self.settings.theme == theme::ThemeChoice::Auto {
            let dark = theme::window_dark(window.appearance());
            theme::apply(theme::palette_for(self.settings.theme, Some(dark)));
            cx.refresh_windows();
        }
        cx.notify();
    }

    fn set_theme(&mut self, choice: theme::ThemeChoice, window: &Window, cx: &mut Context<Self>) {
        self.settings.theme = choice;
        let dark = theme::window_dark(window.appearance());
        theme::apply(theme::palette_for(choice, Some(dark)));
        if self.persist
            && let Err(e) = settings::save(&self.settings)
        {
            log::warn!("could not write {}: {e}", settings::path().display());
        }
        cx.refresh_windows();
        cx.notify();
    }

    fn open_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = cx.new(|cx| TextInput::new("host or host:port", "", cx));
        let password = cx.new(|cx| TextInput::new("VNC password (optional)", "", cx).masked(true));
        window.focus(&address.read(cx).handle());
        self.quick_connect = Some(QuickConnect {
            address,
            password,
            protocol: Protocol::VncTigerVnc,
            notice: None,
        });
        cx.notify();
    }

    fn close_quick_connect(&mut self, cx: &mut Context<Self>) {
        self.quick_connect = None;
        cx.notify();
    }

    fn submit_quick_connect(&mut self, cx: &mut Context<Self>) {
        let Some(quick) = self.quick_connect.clone() else {
            return;
        };
        let text = quick.address.read(cx).content();
        let result = crate::model::parse_address(&text);
        match result {
            Ok((host, port)) => {
                let connection = Connection {
                    name: text.clone(),
                    host,
                    port,
                    protocol: quick.protocol,
                    username: String::new(),
                    last_used: "Just now".into(),
                    online: false,
                    send_clipboard: self.settings.send_clipboard,
                    receive_clipboard: self.settings.receive_clipboard,
                    grab_keyboard: self.settings.grab_keyboard,
                    save_password: false,
                };
                let password = quick.password.read(cx).content();
                self.quick_connect = None;
                self.open_session_deferred(connection, password, cx);
            }
            Err(e) => {
                if let Some(quick) = &mut self.quick_connect {
                    quick.notice = Some(e);
                }
            }
        }
        cx.notify();
    }

    fn delete_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        self.context_menu = None;
        if index < self.connections.len() {
            let address = self.connections[index].address();
            cx.global::<Passwords>().forget(&address);
            if self.persist {
                crate::secrets::forget_password(&address);
            }
            self.connections.remove(index);
        }
        self.selected = None;
        if self.persist
            && let Err(e) = store::save(&self.connections)
        {
            log::warn!("could not write {}: {e}", store::path().display());
        }
        cx.notify();
    }

    fn connect_editor(&mut self, cx: &mut Context<Self>) {
        let password = self
            .editor
            .as_ref()
            .map(|editor| editor.password.read(cx).content())
            .unwrap_or_default();
        let Some(connection) = self.commit_editor(cx) else {
            return;
        };
        self.editor = None;
        self.notice = None;
        self.open_session_deferred(connection, password, cx);
        cx.notify();
    }

    fn delete_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        if let Some(index) = editor.existing {
            let address = self.connections[index].address();
            cx.global::<Passwords>().forget(&address);
            if self.persist {
                crate::secrets::forget_password(&address);
            }
            self.connections.remove(index);
            self.selected = None;
        }
        self.editor = None;
        if self.persist
            && let Err(e) = store::save(&self.connections)
        {
            log::warn!("could not write {}: {e}", store::path().display());
        }
        cx.notify();
    }

    fn online_count(&self) -> usize {
        self.connections.iter().filter(|c| c.online).count()
    }

    fn filtered_indices(&self, connections: &[Connection]) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        connections
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                query.is_empty()
                    || c.name.to_lowercase().contains(&query)
                    || c.host.to_lowercase().contains(&query)
                    || c.protocol.label().to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

impl Render for VentoxApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connections = self.connections.clone();
        let selected = self.selected;
        let editor = self.editor.clone();
        let filtered = self.filtered_indices(&connections);
        let context_menu = self.context_menu;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::window_bg())
            .font_family(theme::FONT_FAMILY)
            .text_color(theme::text())
            .child(self.titlebar())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.sidebar(&connections, &filtered, selected, cx))
                    .child(self.main_area(&connections, &filtered, cx)),
            )
            .children(editor.map(|editor| self.editor_overlay(editor, cx)))
            .children(
                self.quick_connect
                    .clone()
                    .map(|quick| self.quick_connect_overlay(quick, cx)),
            )
            .children(self.settings_open.then(|| {
                let dark = theme::window_dark(window.appearance());
                self.settings_overlay(dark, cx)
            }))
            .children(context_menu.map(|menu| self.context_menu_overlay(menu, window, cx)))
    }
}

impl VentoxApp {
    fn titlebar(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .h(px(38.))
            .flex_shrink_0()
            .bg(theme::sidebar_bg())
            .border_b_1()
            .border_color(theme::border_soft())
            .child(
                div()
                    .id("titlebar-brand")
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h_full()
                    .pl(px(14.))
                    .pr(px(14.))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .rounded_sm()
                            .bg(theme::accent())
                            .text_color(theme::accent_text())
                            .text_size(px(9.5))
                            .font_weight(theme::WEIGHT_BOLD)
                            .child("V"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(theme::WEIGHT_MEDIUM)
                            .text_color(theme::text_secondary())
                            .child("Ventox"),
                    ),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .flex_1()
                    .h_full()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .pr(px(8.))
                    .child(window_button("−", WindowButton::Minimize))
                    .child(window_button("□", WindowButton::Zoom))
                    .child(window_button("✕", WindowButton::Close)),
            )
    }

    fn sidebar(
        &self,
        connections: &[Connection],
        filtered: &[usize],
        selected: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(theme::SIDEBAR_WIDTH)
            .h_full()
            .flex_shrink_0()
            .bg(theme::sidebar_bg())
            .border_r_1()
            .border_color(theme::border_soft())
            // Search
            .child(
                div()
                    .px(px(12.))
                    .pt(px(12.))
                    .pb(px(10.))
                    .child(self.query_input.clone()),
            )
            .child(
                div()
                    .px(px(18.))
                    .pt(px(6.))
                    .pb(px(6.))
                    .text_size(px(9.5))
                    .font_weight(theme::WEIGHT_SEMIBOLD)
                    .text_color(theme::text_tertiary())
                    .child("CONNECTIONS"),
            )
            // Connection list
            .child(
                div()
                    .id("connection-list")
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(8.))
                    .overflow_y_scroll()
                    .children(filtered.iter().map(|&i| {
                        let c = &connections[i];
                        let active = selected == Some(i);
                        div()
                            .id(("connection", i))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .h(px(46.))
                            .px(px(8.))
                            .rounded_md()
                            .cursor_pointer()
                            .when(active, |s| s.bg(theme::surface_active()))
                            .hover(|s| s.bg(theme::surface_hover()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.connect_connection(i, cx);
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.context_menu = Some(ContextMenu {
                                        index: i,
                                        position: event.position,
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(protocol_tile(c.protocol, px(28.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(12.5))
                                            .font_weight(theme::WEIGHT_MEDIUM)
                                            .child(c.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(10.5))
                                            .text_color(theme::text_tertiary())
                                            .child(c.address()),
                                    ),
                            )
                            .when(c.online, |s| {
                                s.child(
                                    div()
                                        .size(px(6.))
                                        .rounded_full()
                                        .bg(theme::green())
                                        .mr(px(2.)),
                                )
                            })
                    })),
            )
            .child(div().flex_1())
            // Footer
            .child(
                div()
                    .border_t_1()
                    .border_color(theme::border_soft())
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .id("settings")
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .h(px(34.))
                            .px(px(10.))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::surface_hover()))
                            .text_size(theme::TEXT_SMALL)
                            .text_color(theme::text_secondary())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                            )
                            .child(div().text_size(px(13.)).child("⚙"))
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(28.))
                            .px(px(10.))
                            .text_size(px(9.5))
                            .text_color(theme::text_tertiary())
                            .child(format!("Ventox {}", env!("CARGO_PKG_VERSION")))
                            .child("GPL-3.0"),
                    ),
            )
    }

    fn main_area(
        &self,
        connections: &[Connection],
        filtered: &[usize],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            // Header
            .child(
                div()
                    .flex()
                    .items_end()
                    .justify_between()
                    .px(theme::CONTENT_PADDING)
                    .pt(px(28.))
                    .pb(px(20.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(theme::TEXT_LARGE_TITLE)
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .child("Connections"),
                            )
                            .child(
                                div()
                                    .text_size(theme::TEXT_SMALL)
                                    .text_color(theme::text_tertiary())
                                    .child(format!(
                                        "{} saved · {} online",
                                        connections.len(),
                                        self.online_count()
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(quick_connect_button(cx))
                            .child(new_connection_button(cx)),
                    ),
            )
            // Cards
            .child(
                div()
                    .id("connection-cards")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(theme::CONTENT_PADDING)
                    .pb(px(28.))
                    .child(if filtered.is_empty() {
                        empty_state(!self.query.trim().is_empty(), cx)
                    } else {
                        let cards: Vec<AnyElement> = filtered
                            .iter()
                            .map(|&i| connection_card(connections, i, cx).into_any_element())
                            .collect();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(16.))
                            .children(cards)
                            .into_any_element()
                    }),
            )
    }

    fn context_menu_overlay(
        &self,
        menu: ContextMenu,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(connection) = self.connections.get(menu.index) else {
            return div().into_any_element();
        };
        let size = window.bounds().size;
        let window_w: f32 = size.width.into();
        let window_h: f32 = size.height.into();
        let x = f32::from(menu.position.x);
        let y = f32::from(menu.position.y);
        let menu_w = 208.0;
        let menu_h = 150.0;
        let left = if x + menu_w + 8.0 > window_w {
            (x - menu_w).max(8.0)
        } else {
            x
        };
        let top = if y + menu_h + 8.0 > window_h {
            (y - menu_h).max(8.0)
        } else {
            y
        };

        div()
            .id("context-scrim")
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("context-menu")
                    .occlude()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(menu_w))
                    .py(px(6.))
                    .rounded_lg()
                    .bg(theme::surface_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_lg()
                    .child(
                        div()
                            .px(px(14.))
                            .pt(px(4.))
                            .pb(px(8.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.5))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .child(connection.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(10.))
                                    .text_color(theme::text_tertiary())
                                    .child(format!(
                                        "{} · {}",
                                        connection.protocol.label(),
                                        connection.address()
                                    )),
                            ),
                    )
                    .child(context_menu_item("Connect", 0, false, menu.index, cx))
                    .child(context_menu_item("Edit", 1, false, menu.index, cx))
                    .child(
                        div()
                            .mx(px(8.))
                            .my(px(4.))
                            .h(px(1.))
                            .bg(theme::border_soft()),
                    )
                    .child(context_menu_item("Delete", 2, true, menu.index, cx)),
            )
            .into_any_element()
    }

    fn quick_connect_overlay(&self, quick: QuickConnect, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("quick-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_quick_connect(cx)),
            )
            .child(
                div()
                    .id("quick-sheet")
                    .occlude()
                    .flex()
                    .flex_col()
                    .w(px(460.))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(theme::surface_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_lg()
                    .on_click(|_, _, _| {})
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "enter" | "return" => this.submit_quick_connect(cx),
                            "escape" => this.close_quick_connect(cx),
                            _ => {}
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .px(px(22.))
                            .pt(px(20.))
                            .pb(px(14.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_weight(theme::WEIGHT_SEMIBOLD)
                                            .child("Quick Connect"),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme::TEXT_SMALL)
                                            .text_color(theme::text_tertiary())
                                            .child("Connect without saving a connection"),
                                    ),
                            )
                            .child(quick_close_button(cx)),
                    )
                    .children(quick.notice.as_ref().map(|notice| {
                        div()
                            .mx(px(22.))
                            .mb(px(4.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded_md()
                            .bg(theme::fade(theme::red(), 0.12))
                            .text_size(px(11.5))
                            .text_color(theme::red())
                            .child(notice.clone())
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .px(px(22.))
                            .pb(px(18.))
                            .child(field("ADDRESS", quick.address.clone()))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme::text_tertiary())
                                    .child("Port defaults to 5900."),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(section_label("BACKEND"))
                                    .child(div().flex().gap(px(6.)).children(
                                        Protocol::VNC_IMPLEMENTATIONS.into_iter().map(|protocol| {
                                            quick_protocol_pill(protocol, quick.protocol, cx)
                                        }),
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(field("PASSWORD", quick.password.clone()))
                                    .child(password_hint()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(22.))
                            .py(px(14.))
                            .border_t_1()
                            .border_color(theme::border_soft())
                            .child(quick_cancel_button(cx))
                            .child(
                                div()
                                    .id("quick-submit")
                                    .flex()
                                    .items_center()
                                    .h(px(32.))
                                    .px(px(16.))
                                    .rounded_lg()
                                    .bg(theme::accent())
                                    .hover(|s| s.bg(theme::accent_hover()))
                                    .cursor_pointer()
                                    .text_size(px(12.5))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .text_color(theme::accent_text())
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.submit_quick_connect(cx)),
                                    )
                                    .child("Connect"),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn settings_overlay(&self, dark: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("settings-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::overlay_scrim())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.settings_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("settings-sheet")
                    .occlude()
                    .flex()
                    .flex_col()
                    .w(px(640.))
                    .max_h(px(720.))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(theme::surface_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_lg()
                    .on_click(|_, _, _| {})
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .px(px(24.))
                            .pt(px(22.))
                            .pb(px(14.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(px(19.))
                                            .font_weight(theme::WEIGHT_SEMIBOLD)
                                            .child("Settings"),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme::TEXT_SMALL)
                                            .text_color(theme::text_tertiary())
                                            .child("Appearance and versions"),
                                    ),
                            )
                            .child(quick_close_button(cx)),
                    )
                    .child(
                        div()
                            .id("settings-rows")
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .px(px(24.))
                            .pb(px(18.))
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .text_color(theme::text_tertiary())
                                    .child("APPEARANCE"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(10.))
                                    .children(theme::ThemeChoice::ALL.into_iter().map(|choice| {
                                        theme_card(choice, self.settings.theme, dark, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme::text_tertiary())
                                    .child(
                                        "Auto uses omarchy's wallpaper colors, falling back to system light/dark.",
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .text_color(theme::text_tertiary())
                                    .child("NEW CONNECTIONS"),
                            )
                            .child(settings_toggle_row(
                                "Send clipboard",
                                self.settings.send_clipboard,
                                0,
                                cx,
                            ))
                            .child(settings_toggle_row(
                                "Receive clipboard",
                                self.settings.receive_clipboard,
                                1,
                                cx,
                            ))
                            .child(settings_toggle_row(
                                "Grab keyboard",
                                self.settings.grab_keyboard,
                                2,
                                cx,
                            ))
                            .child(
                                div()
                                    .my(px(2.))
                                    .h(px(1.))
                                    .bg(theme::border_soft()),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .text_color(theme::text_tertiary())
                                    .child("ABOUT"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(theme::text())
                                            .child("Free software — source: github.com/jianlongliu/vento"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(theme::text_tertiary())
                                            .child("If you paid for it, you were overcharged."),
                                    ),
                            )
                            .children(about::rows().into_iter().map(|(label, value)| {
                                div()
                                    .flex()
                                    .items_start()
                                    .justify_between()
                                    .gap(px(16.))
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .w(px(150.))
                                            .text_size(px(11.))
                                            .text_color(theme::text_tertiary())
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(11.5))
                                            .text_color(theme::text_secondary())
                                            .child(value),
                                    )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .px(px(24.))
                            .py(px(14.))
                            .border_t_1()
                            .border_color(theme::border_soft())
                            .child(
                                div()
                                    .id("settings-close")
                                    .flex()
                                    .items_center()
                                    .h(px(32.))
                                    .px(px(16.))
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::surface_hover()))
                                    .text_size(px(12.5))
                                    .text_color(theme::text_secondary())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_open = false;
                                        cx.notify();
                                    }))
                                    .child("Close"),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn editor_overlay(&self, editor: EditorState, cx: &mut Context<Self>) -> AnyElement {
        let is_new = editor.existing.is_none();
        div()
            .id("editor-scrim")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::overlay_scrim())
            .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx)))
            .child(
                div()
                    .id("editor-sheet")
                    .occlude()
                    .flex()
                    .flex_col()
                    .w(px(600.))
                    .max_h(px(760.))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(theme::surface_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_lg()
                    .on_click(|_, _, _| {})
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "tab" => {
                                this.editor_focus_next(event.keystroke.modifiers.shift, window, cx)
                            }
                            "enter" | "return" => {
                                if this.editor_password_focused(window, cx) {
                                    this.connect_editor(cx);
                                } else {
                                    this.editor_focus_next(false, window, cx);
                                }
                            }
                            "escape" => this.close_editor(cx),
                            _ => {}
                        }
                    }))
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .px(px(24.))
                            .pt(px(22.))
                            .pb(px(16.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(px(19.))
                                            .font_weight(theme::WEIGHT_SEMIBOLD)
                                            .child(if is_new {
                                                "New Connection"
                                            } else {
                                                "Edit Connection"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme::TEXT_SMALL)
                                            .text_color(theme::text_tertiary())
                                            .child("Connect to a remote desktop"),
                                    ),
                            )
                            .child(close_button(cx)),
                    )
                    .children(self.notice.as_ref().map(|notice| {
                        div()
                            .mx(px(24.))
                            .mb(px(4.))
                            .px(px(12.))
                            .py(px(8.))
                            .rounded_md()
                            .bg(theme::fade(theme::red(), 0.12))
                            .text_size(px(11.5))
                            .text_color(theme::red())
                            .child(notice.clone())
                    }))
                    // Body
                    .child(
                        div()
                            .id("editor-body")
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .px(px(24.))
                            .pb(px(20.))
                            .overflow_y_scroll()
                            .child(field("PROTOCOL", protocol_picker(editor.protocol, cx)))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(10.))
                                    .child(field("NAME", editor.name.clone()))
                                    .child(field("USERNAME", editor.username.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap(px(10.))
                                    .child(field("ADDRESS", editor.host.clone()))
                                    .child(field_fixed("PORT", px(104.), editor.port.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(field("PASSWORD", editor.password.clone()))
                                    .child(password_hint()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.))
                                    .child(section_label("OPTIONS"))
                                    .child(toggle_row(
                                        "Send clipboard",
                                        editor.send_clipboard,
                                        0,
                                        cx,
                                    ))
                                    .child(toggle_row(
                                        "Receive clipboard",
                                        editor.receive_clipboard,
                                        1,
                                        cx,
                                    ))
                                    .child(toggle_row("Grab keyboard", editor.grab_keyboard, 2, cx))
                                    .child(toggle_row(
                                        "Save password",
                                        editor.save_password,
                                        3,
                                        cx,
                                    )),
                            ),
                    )
                    // Footer
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(24.))
                            .py(px(14.))
                            .border_t_1()
                            .border_color(theme::border_soft())
                            .child(div().when(!is_new, |s| {
                                s.child(
                                    div()
                                        .id("delete-connection")
                                        .flex()
                                        .items_center()
                                        .h(px(32.))
                                        .px(px(12.))
                                        .rounded_lg()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgba(0xff453a1a)))
                                        .text_size(px(12.5))
                                        .text_color(theme::red())
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.delete_editor(cx)),
                                        )
                                        .child("Delete"),
                                )
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(ghost_button("Cancel", cx))
                                    .child(
                                        div()
                                            .id("connect-connection")
                                            .flex()
                                            .items_center()
                                            .h(px(32.))
                                            .px(px(16.))
                                            .rounded_lg()
                                            .bg(theme::accent())
                                            .hover(|s| s.bg(theme::accent_hover()))
                                            .cursor_pointer()
                                            .text_size(px(12.5))
                                            .font_weight(theme::WEIGHT_SEMIBOLD)
                                            .text_color(theme::accent_text())
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.connect_editor(cx)
                                                }),
                                            )
                                            .child("Save and Connect"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy)]
enum WindowButton {
    Minimize,
    Zoom,
    Close,
}

fn window_button(glyph: &'static str, kind: WindowButton) -> AnyElement {
    let close = matches!(kind, WindowButton::Close);
    let id = match kind {
        WindowButton::Minimize => "window-minimize",
        WindowButton::Zoom => "window-zoom",
        WindowButton::Close => "window-close",
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded_md()
        .cursor_pointer()
        .text_size(px(11.))
        .text_color(theme::text_secondary())
        .hover(move |s| {
            if close {
                s.bg(theme::red()).text_color(theme::text())
            } else {
                s.bg(theme::surface_hover()).text_color(theme::text())
            }
        })
        .on_mouse_down(MouseButton::Left, move |_, window, cx| match kind {
            WindowButton::Minimize => window.minimize_window(),
            WindowButton::Zoom => window.zoom_window(),
            WindowButton::Close => cx.quit(),
        })
        .child(glyph)
        .into_any_element()
}

fn theme_card(
    choice: theme::ThemeChoice,
    selected: theme::ThemeChoice,
    dark: bool,
    cx: &mut Context<VentoxApp>,
) -> AnyElement {
    let palette = theme::palette_for(choice, Some(dark));
    let active = choice == selected;
    div()
        .id(("theme", choice as usize))
        .flex()
        .flex_col()
        .gap(px(7.))
        .w(px(285.))
        .p(px(12.))
        .rounded_lg()
        .bg(palette.surface)
        .border_1()
        .border_color(if active {
            palette.accent
        } else {
            palette.border
        })
        .cursor_pointer()
        .hover(|s| s.border_color(theme::accent()))
        .on_click(cx.listener(move |this, _, window, cx| this.set_theme(choice, window, cx)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(theme::WEIGHT_SEMIBOLD)
                        .text_color(palette.text)
                        .child(choice.label()),
                )
                .when(active, |s| {
                    s.child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette.accent)
                            .child("●"),
                    )
                }),
        )
        .child(
            div()
                .text_size(px(9.5))
                .text_color(palette.text_secondary)
                .child(choice.description()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(swatch(palette.accent))
                .child(swatch(palette.protocol_tigervnc))
                .child(swatch(palette.green))
                .child(swatch(palette.yellow))
                .child(swatch(palette.red))
                .child(div().flex_1().h(px(12.)).rounded_sm().bg(palette.window_bg)),
        )
        .into_any_element()
}

fn swatch(color: gpui::Rgba) -> AnyElement {
    div()
        .size(px(12.))
        .rounded_sm()
        .bg(color)
        .into_any_element()
}

fn quick_connect_button(cx: &mut Context<VentoxApp>) -> AnyElement {
    div()
        .id("quick-connect")
        .flex()
        .items_center()
        .h(px(34.))
        .px(px(14.))
        .rounded_lg()
        .bg(theme::surface())
        .border_1()
        .border_color(theme::border())
        .hover(|s| s.bg(theme::surface_hover()))
        .cursor_pointer()
        .text_size(px(12.5))
        .font_weight(theme::WEIGHT_MEDIUM)
        .text_color(theme::text_secondary())
        .on_click(cx.listener(|this, _, window, cx| this.open_quick_connect(window, cx)))
        .child("Quick Connect")
        .into_any_element()
}

fn quick_close_button(cx: &mut Context<VentoxApp>) -> AnyElement {
    div()
        .id("quick-close")
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .text_size(px(13.))
        .text_color(theme::text_secondary())
        .on_click(cx.listener(|this, _, _, cx| this.close_quick_connect(cx)))
        .child("✕")
        .into_any_element()
}

fn quick_cancel_button(cx: &mut Context<VentoxApp>) -> AnyElement {
    div()
        .id("quick-cancel")
        .flex()
        .items_center()
        .h(px(32.))
        .px(px(14.))
        .rounded_lg()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .text_size(px(12.5))
        .text_color(theme::text_secondary())
        .on_click(cx.listener(|this, _, _, cx| this.close_quick_connect(cx)))
        .child("Cancel")
        .into_any_element()
}

fn quick_protocol_pill(
    protocol: Protocol,
    selected: Protocol,
    cx: &mut Context<VentoxApp>,
) -> AnyElement {
    let active = protocol == selected;
    div()
        .id(("quick-protocol", protocol as usize))
        .flex()
        .flex_1()
        .flex_col()
        .gap(px(1.))
        .px(px(9.))
        .py(px(7.))
        .rounded_md()
        .bg(if active {
            theme::fade(protocol.color(), 0.10)
        } else {
            theme::surface()
        })
        .border_1()
        .border_color(if active {
            theme::fade(protocol.color(), 0.65)
        } else {
            theme::border()
        })
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(quick) = &mut this.quick_connect {
                quick.protocol = protocol;
            }
            cx.notify();
        }))
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(theme::WEIGHT_SEMIBOLD)
                .text_color(if active {
                    protocol.color()
                } else {
                    theme::text()
                })
                .child(protocol.label()),
        )
        .child(
            div()
                .truncate()
                .text_size(px(9.))
                .text_color(theme::text_tertiary())
                .child(protocol.description()),
        )
        .into_any_element()
}

fn settings_toggle_row(
    label: &'static str,
    on: bool,
    kind: u8,
    cx: &mut Context<VentoxApp>,
) -> AnyElement {
    div()
        .id(("settings-toggle", kind as usize))
        .flex()
        .items_center()
        .justify_between()
        .h(px(28.))
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .child(
            div()
                .px(px(4.))
                .text_size(px(12.))
                .text_color(theme::text_secondary())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .mr(px(4.))
                .w(px(34.))
                .h(px(20.))
                .p(px(2.))
                .rounded_full()
                .bg(if on {
                    theme::accent()
                } else {
                    theme::surface_active()
                })
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .bg(theme::text())
                        .when(on, |s| s.ml(px(14.))),
                ),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            match kind {
                0 => this.settings.send_clipboard = !this.settings.send_clipboard,
                1 => this.settings.receive_clipboard = !this.settings.receive_clipboard,
                _ => this.settings.grab_keyboard = !this.settings.grab_keyboard,
            }
            if this.persist
                && let Err(e) = settings::save(&this.settings)
            {
                log::warn!("could not write {}: {e}", settings::path().display());
            }
            cx.notify();
        }))
        .into_any_element()
}

fn password_hint() -> AnyElement {
    div()
        .text_size(px(10.5))
        .text_color(theme::text_tertiary())
        .child("For VNC, leave Username empty and enter the VNC password here.")
        .into_any_element()
}

fn new_connection_button(cx: &mut Context<VentoxApp>) -> AnyElement {
    div()
        .id("new-connection")
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(34.))
        .px(px(14.))
        .rounded_lg()
        .bg(theme::accent())
        .hover(|s| s.bg(theme::accent_hover()))
        .cursor_pointer()
        .text_size(px(12.5))
        .font_weight(theme::WEIGHT_SEMIBOLD)
        .text_color(theme::accent_text())
        .on_click(cx.listener(|this, _, window, cx| this.open_new_editor(window, cx)))
        .child(div().text_size(px(15.)).child("+"))
        .child("New Connection")
        .into_any_element()
}

fn empty_state(searching: bool, cx: &mut Context<VentoxApp>) -> AnyElement {
    div()
        .size_full()
        .min_h(px(380.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(64.))
                .rounded_xl()
                .bg(theme::fade(theme::accent(), 0.12))
                .text_size(px(26.))
                .font_weight(theme::WEIGHT_BOLD)
                .text_color(theme::accent())
                .child("V"),
        )
        .child(
            div()
                .mt(px(4.))
                .text_size(px(17.))
                .font_weight(theme::WEIGHT_SEMIBOLD)
                .child(if searching {
                    "No matches"
                } else {
                    "No connections yet"
                }),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme::text_secondary())
                .child(if searching {
                    "Nothing in your library matches that search."
                } else {
                    "Add a remote desktop to get started."
                }),
        )
        .children(if searching {
            Some(
                div()
                    .id("clear-search")
                    .mt(px(8.))
                    .flex()
                    .items_center()
                    .h(px(32.))
                    .px(px(14.))
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::border())
                    .hover(|s| s.bg(theme::surface_hover()))
                    .text_size(px(12.))
                    .text_color(theme::text_secondary())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.query_input
                            .update(cx, |input, cx| input.set_content("", cx));
                        this.query.clear();
                        cx.notify();
                    }))
                    .child("Clear search")
                    .into_any_element(),
            )
        } else {
            Some(
                div()
                    .mt(px(8.))
                    .child(new_connection_button(cx))
                    .into_any_element(),
            )
        })
        .into_any_element()
}

fn context_menu_item(
    label: &'static str,
    kind: u8,
    danger: bool,
    index: usize,
    cx: &mut Context<VentoxApp>,
) -> AnyElement {
    div()
        .id(("context-menu-item", kind as usize))
        .flex()
        .items_center()
        .h(px(30.))
        .mx(px(4.))
        .px(px(10.))
        .rounded_md()
        .cursor_pointer()
        .text_size(px(12.))
        .text_color(if danger { theme::red() } else { theme::text() })
        .hover(move |s| {
            if danger {
                s.bg(theme::fade(theme::red(), 0.12))
            } else {
                s.bg(theme::surface_hover())
            }
        })
        .on_click(cx.listener(move |this, _, window, cx| match kind {
            0 => this.connect_connection(index, cx),
            1 => this.open_editor(index, window, cx),
            _ => this.delete_connection(index, cx),
        }))
        .child(label)
        .into_any_element()
}

fn connection_card(
    connections: &[Connection],
    index: usize,
    cx: &mut Context<VentoxApp>,
) -> impl IntoElement {
    let connection = &connections[index];
    div()
        .id(("card", index))
        .flex()
        .flex_col()
        .w(theme::CARD_WIDTH)
        .rounded_xl()
        .overflow_hidden()
        .bg(theme::surface())
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .hover(|s| {
            s.bg(theme::surface_hover())
                .border_color(theme::border_strong())
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.connect_connection(index, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.context_menu = Some(ContextMenu {
                    index,
                    position: event.position,
                });
                cx.notify();
            }),
        )
        .child(preview(connection))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .px(px(14.))
                .py(px(12.))
                .child(
                    div()
                        .truncate()
                        .text_size(theme::TEXT_BODY)
                        .font_weight(theme::WEIGHT_SEMIBOLD)
                        .child(connection.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .truncate()
                                .text_size(theme::TEXT_MICRO)
                                .font_family(theme::MONO_FAMILY)
                                .text_color(theme::text_tertiary())
                                .child(connection.address()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(9.5))
                                .text_color(theme::text_tertiary())
                                .child(connection.last_used.clone()),
                        ),
                ),
        )
}

fn protocol_tile(protocol: Protocol, size: gpui::Pixels) -> impl IntoElement {
    let mark = match protocol {
        Protocol::VncRust => "RV",
        Protocol::VncLibVnc => "LV",
        Protocol::VncTigerVnc => "TV",
        Protocol::RdpFreeRdp => "FD",
        Protocol::RdpIronRdp => "IR",
        Protocol::Spice => "S",
        Protocol::RustDesk => "RD",
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .size(size)
        .rounded_md()
        .bg(theme::fade(protocol.color(), 0.16))
        .text_size(px(
            if matches!(
                protocol,
                Protocol::RustDesk | Protocol::RdpFreeRdp | Protocol::RdpIronRdp
            ) {
                8.5
            } else {
                9.5
            },
        ))
        .font_weight(theme::WEIGHT_BOLD)
        .text_color(protocol.color())
        .child(mark)
}

fn protocol_chip(protocol: Protocol) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .px(px(6.))
        .py(px(2.))
        .rounded_sm()
        .bg(theme::fade(protocol.color(), 0.14))
        .text_size(px(9.))
        .font_weight(theme::WEIGHT_SEMIBOLD)
        .text_color(protocol.color())
        .child(protocol.label())
}

fn preview(connection: &Connection) -> impl IntoElement {
    let color = connection.protocol.color();
    div()
        .relative()
        .h(px(104.))
        .w_full()
        .overflow_hidden()
        .bg(theme::surface())
        .child(div().absolute().inset_0().bg(theme::fade(color, 0.10)))
        .child(
            div()
                .absolute()
                .right(px(-18.))
                .bottom(px(-26.))
                .text_size(px(104.))
                .font_weight(theme::WEIGHT_BOLD)
                .text_color(theme::fade(color, 0.08))
                .child(connection.protocol.label()),
        )
        .child(
            div()
                .absolute()
                .left(px(14.))
                .top(px(12.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .px(px(7.))
                        .py(px(2.))
                        .rounded_sm()
                        .bg(rgba(0x00000033))
                        .text_size(px(9.))
                        .font_weight(theme::WEIGHT_SEMIBOLD)
                        .text_color(color)
                        .child(connection.protocol.label()),
                )
                .when(connection.online, |s| {
                    s.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded_sm()
                            .bg(rgba(0x00000033))
                            .text_size(px(9.))
                            .text_color(theme::green())
                            .child(div().size(px(4.)).rounded_full().bg(theme::green()))
                            .child("Online"),
                    )
                }),
        )
        .child(
            div()
                .absolute()
                .left(px(14.))
                .bottom(px(12.))
                .right(px(14.))
                .flex()
                .items_center()
                .gap(px(7.))
                .child(protocol_chip(connection.protocol))
                .child(
                    div()
                        .truncate()
                        .text_size(px(10.5))
                        .text_color(theme::text_secondary())
                        .child(connection.protocol.description()),
                ),
        )
}

fn field(label: &'static str, child: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .flex_1()
        .min_w(px(0.))
        .child(section_label(label))
        .child(child)
}

fn field_fixed(
    label: &'static str,
    width: gpui::Pixels,
    child: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .w(width)
        .flex_shrink_0()
        .child(section_label(label))
        .child(child)
}

fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .text_size(px(9.5))
        .font_weight(theme::WEIGHT_SEMIBOLD)
        .text_color(theme::text_tertiary())
        .child(label)
}

fn protocol_picker(selected: Protocol, cx: &mut Context<VentoxApp>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div().flex().gap(px(6.)).children(
                Protocol::VNC_IMPLEMENTATIONS
                    .into_iter()
                    .map(|protocol| protocol_option(protocol, selected, cx)),
            ),
        )
        .child(
            div().flex().gap(px(6.)).children(
                Protocol::RDP_IMPLEMENTATIONS
                    .into_iter()
                    .chain(Protocol::OTHER_PROTOCOLS)
                    .map(|protocol| protocol_option(protocol, selected, cx)),
            ),
        )
}

fn protocol_option(
    protocol: Protocol,
    selected: Protocol,
    cx: &mut Context<VentoxApp>,
) -> AnyElement {
    let active = protocol == selected;
    div()
        .id(("protocol", protocol as usize))
        .flex()
        .flex_1()
        .flex_col()
        .gap(px(2.))
        .px(px(9.))
        .py(px(7.))
        .rounded_md()
        .bg(if active {
            theme::fade(protocol.color(), 0.10)
        } else {
            theme::surface()
        })
        .border_1()
        .border_color(if active {
            theme::fade(protocol.color(), 0.65)
        } else {
            theme::border()
        })
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(editor) = &mut this.editor {
                editor.protocol = protocol;
                editor.port.update(cx, |input, cx| {
                    input.set_content(protocol.default_port().to_string(), cx)
                });
            }
            cx.notify();
        }))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(theme::WEIGHT_SEMIBOLD)
                .text_color(if active {
                    protocol.color()
                } else {
                    theme::text()
                })
                .child(protocol.label()),
        )
        .child(
            div()
                .truncate()
                .text_size(px(9.))
                .text_color(theme::text_tertiary())
                .child(protocol.description()),
        )
        .into_any_element()
}

fn toggle_row(
    label: &'static str,
    on: bool,
    kind: u8,
    cx: &mut Context<VentoxApp>,
) -> impl IntoElement {
    div()
        .id(("editor-toggle", kind as usize))
        .flex()
        .items_center()
        .justify_between()
        .h(px(30.))
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .child(
            div()
                .px(px(4.))
                .text_size(px(12.))
                .text_color(theme::text_secondary())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .mr(px(4.))
                .w(px(34.))
                .h(px(20.))
                .p(px(2.))
                .rounded_full()
                .bg(if on {
                    theme::accent()
                } else {
                    theme::surface_active()
                })
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .bg(theme::text())
                        .when(on, |s| s.ml(px(14.))),
                ),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(editor) = &mut this.editor {
                match kind {
                    0 => editor.send_clipboard = !editor.send_clipboard,
                    1 => editor.receive_clipboard = !editor.receive_clipboard,
                    2 => editor.grab_keyboard = !editor.grab_keyboard,
                    _ => editor.save_password = !editor.save_password,
                }
            }
            cx.notify();
        }))
}

fn close_button(cx: &mut Context<VentoxApp>) -> impl IntoElement {
    div()
        .id("close-editor")
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .text_size(px(13.))
        .text_color(theme::text_secondary())
        .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx)))
        .child("✕")
}

fn ghost_button(label: &'static str, cx: &mut Context<VentoxApp>) -> impl IntoElement {
    div()
        .id("cancel")
        .flex()
        .items_center()
        .h(px(32.))
        .px(px(14.))
        .rounded_lg()
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_hover()))
        .text_size(px(12.5))
        .text_color(theme::text_secondary())
        .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx)))
        .child(label)
}
