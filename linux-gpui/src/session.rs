// SPDX-License-Identifier: GPL-3.0-or-later
//
//! A live remote desktop session in its own window.
//!
//! The protocol runs on the backend's own thread and reports through the
//! mailbox. A wake channel turns each report into a GPUI task update; frames
//! are read from the backend's framebuffer into a `RenderImage`, and the
//! chrome around the picture stays out of its way: a floating toolbar, an
//! optional information panel, and sheets for credentials and trust.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClipboardItem, Context, FocusHandle, IntoElement, KeyDownEvent, KeyUpEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, RenderImage, ScrollDelta,
    ScrollWheelEvent, Window, div, img, px, rgb, rgba,
};

use api::Credentials;
use api::backend::{Backend, Facts};
use api::events::{Event, Mailbox};
use api::kind::{Connect, Dirs, Platform};
use api::options::Options;

use crate::keys;
use crate::model::{Connection, Protocol};
use crate::passwords::Passwords;
use crate::store;
use crate::text_input::TextInput;
use crate::theme;

pub fn open(
    connection: Connection,
    password: String,
    persist: bool,
    cx: &mut App,
) -> Result<(), String> {
    let (wake_tx, wake_rx) = smol::channel::unbounded::<()>();
    let mailbox = Mailbox::new(Some(Box::new(move || {
        let _ = wake_tx.try_send(());
    })));

    let kind = match connection.protocol {
        Protocol::VncTigerVnc => &remotedesktop_tigervnc::KIND,
        Protocol::VncLibVnc => &remotedesktop_libvnc::KIND,
        Protocol::VncRust => &remotedesktop_rfb::backend::KIND,
        Protocol::RdpFreeRdp => &remotedesktop_freerdp::KIND,
        Protocol::RdpIronRdp => &remotedesktop_ironrdp::backend::KIND,
        Protocol::Spice => &remotedesktop_spice::backend::KIND,
        Protocol::RustDesk => {
            return Err(format!(
                "{} is not wired up yet",
                Protocol::RustDesk.label()
            ));
        }
    };
    let mut options = Options::defaults(kind.options);
    for (key, value) in [("ViewOnly", "false"), ("Shared", "true")] {
        if kind.options.iter().any(|spec| spec.key == key) {
            options.set(key, value);
        }
    }

    let state_dir = store::state_dir();
    let dirs = Dirs {
        config: state_dir.clone(),
        state: state_dir,
        realvnc_libs: store::data_dir().join("realvnc"),
    };
    let address = connection.address();
    let client_name = client_name();
    // The editor's password wins; otherwise reuse one entered earlier in
    // this run, so a card click does not prompt twice.
    let password = if password.is_empty() {
        cx.try_global::<Passwords>()
            .and_then(|passwords| passwords.get(&address))
            .or_else(|| {
                if connection.save_password && persist {
                    crate::secrets::load_password(&address)
                } else {
                    None
                }
            })
    } else {
        Some(password)
    };
    if let Some(password) = &password {
        log::debug!("password for {address}: reused");
        cx.global::<Passwords>().set(&address, password);
    } else {
        log::debug!("password for {address}: will prompt");
    }
    let connect = Connect {
        address: &address,
        user: &connection.username,
        domain: "",
        password: password.as_deref(),
        options: &options,
        mailbox: Arc::clone(&mailbox),
        dirs: &dirs,
        screen: None,
        last_size: None,
        identity: None,
        client_name: &client_name,
        platform: Platform::Linux,
        decoders: None,
        audio: None,
    };
    let backend = kind.start(&connect)?;

    let bounds = gpui::Bounds::centered(None, gpui::size(px(1280.), px(820.)), cx);
    cx.open_window(
        gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
            window_min_size: Some(gpui::size(px(640.), px(420.))),
            window_decorations: Some(gpui::WindowDecorations::Client),
            app_id: Some("dev.ventox.Ventox".into()),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(connection.name.clone().into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            let session =
                cx.new(|cx| SessionWindow::new(cx, connection, backend, mailbox, persist, wake_rx));
            let handle = session.read(cx).focus_handle.clone();
            window.focus(&handle);
            session
        },
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn client_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "ventox".into())
}

#[derive(Debug, PartialEq)]
struct ImageLayout {
    scale: f32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScaleMode {
    /// Fit, but never enlarged past one desktop pixel per device pixel.
    Auto,
    /// Fill the window, even if that enlarges a small desktop.
    Fit,
    /// One desktop pixel per device pixel (sharp, may leave bars).
    Actual,
}

enum State {
    Connecting,
    Connected,
    Closed { detail: String },
}

enum Prompt {
    Credentials,
    Trust {
        title: String,
        detail: String,
    },
    Message {
        text: String,
        confirm: String,
        question: bool,
    },
}

pub struct SessionWindow {
    connection: Connection,
    backend: Arc<dyn Backend>,
    _mailbox: Arc<Mailbox>,
    state: State,
    status: Option<String>,
    facts: Facts,
    image: Option<Arc<RenderImage>>,
    old_images: VecDeque<Arc<RenderImage>>,
    buffer: Vec<u8>,
    prompt: Option<Prompt>,
    focus_password: bool,
    password: gpui::Entity<TextInput>,
    info_open: bool,
    scale: ScaleMode,
    last_frame: Option<Instant>,
    send_clipboard: bool,
    receive_clipboard: bool,
    grab_keyboard: bool,
    save_password: bool,
    focus_handle: FocusHandle,
    buttons: i32,
    pointer: (i32, i32),
    synthetic_modifiers: HashMap<i64, Vec<(i64, u32)>>,
}

impl SessionWindow {
    fn new(
        cx: &mut Context<Self>,
        connection: Connection,
        backend: Arc<dyn Backend>,
        mailbox: Arc<Mailbox>,
        persist: bool,
        wake_rx: smol::channel::Receiver<()>,
    ) -> Self {
        let send_clipboard = connection.send_clipboard;
        let receive_clipboard = connection.receive_clipboard;
        let grab_keyboard = connection.grab_keyboard;
        let save_password = connection.save_password;
        let password = cx.new(|cx| TextInput::new("Password", "", cx).masked(true));
        let focus_handle = cx.focus_handle();

        let task_mailbox = Arc::clone(&mailbox);
        cx.spawn(async move |this, cx| {
            while wake_rx.recv().await.is_ok() {
                let events = task_mailbox.take_all();
                if this
                    .update(cx, |this, cx| this.handle_events(events, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            connection,
            backend,
            _mailbox: mailbox,
            state: State::Connecting,
            status: None,
            facts: Facts::NONE,
            image: None,
            old_images: VecDeque::new(),
            buffer: Vec::new(),
            prompt: None,
            focus_password: false,
            password,
            info_open: false,
            scale: ScaleMode::Auto,
            last_frame: None,
            send_clipboard,
            receive_clipboard,
            grab_keyboard,
            save_password,
            focus_handle,
            buttons: 0,
            pointer: (0, 0),
            synthetic_modifiers: HashMap::new(),
        }
    }

    fn handle_events(&mut self, events: Vec<Event>, cx: &mut Context<Self>) {
        let mut frame = false;
        for event in events {
            match event {
                Event::Connected { .. } | Event::DesktopSize { .. } => {
                    self.facts = self.backend.facts();
                    self.state = State::Connected;
                    self.status = None;
                }
                Event::Frame { .. } => frame = true,
                Event::Status(status) => self.status = status,
                Event::Closed { detail } => {
                    self.state = State::Closed { detail };
                    self.status = None;
                    self.prompt = None;
                }
                Event::CredentialsNeeded { .. } => {
                    self.prompt = Some(Prompt::Credentials);
                    self.focus_password = true;
                }
                Event::PromptWithdrawn => self.prompt = None,
                Event::TrustNeeded { fingerprint } => {
                    self.prompt = Some(Prompt::Trust {
                        title: "Unrecognised server".into(),
                        detail: format!("SHA-256 fingerprint\n{fingerprint}"),
                    });
                }
                Event::Unverified { why } => {
                    self.prompt = Some(Prompt::Trust {
                        title: "Server not verified".into(),
                        detail: why,
                    });
                }
                Event::Question { title, text } => {
                    self.prompt = Some(Prompt::Trust {
                        title,
                        detail: text,
                    });
                }
                Event::SecurityNeeded {
                    identity,
                    encryption,
                    signature,
                    catchphrase,
                    matching_address,
                } => {
                    self.prompt = Some(Prompt::Trust {
                        title: "Verify server identity".into(),
                        detail: format!(
                            "{identity:?} identity\n{encryption:?}\n\nSignature: {signature}\nCatchphrase: {catchphrase}\nAddress: {matching_address}"
                        ),
                    });
                }
                Event::Message {
                    text,
                    question,
                    confirm_label,
                    ..
                } => {
                    self.prompt = Some(Prompt::Message {
                        text,
                        confirm: confirm_label,
                        question,
                    });
                }
                Event::Clipboard(text) => {
                    if self.receive_clipboard {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                _ => {}
            }
        }
        if frame {
            self.read_frame(cx);
        }
        cx.notify();
    }

    fn read_frame(&mut self, cx: &mut Context<Self>) {
        let width = self.facts.width;
        let height = self.facts.height;
        if width == 0 || height == 0 {
            return;
        }
        let stride = width * 4;
        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.resize(stride * height, 0);
        if !self
            .backend
            .read_region(0, 0, width, height, &mut buffer, stride, 0, 0)
        {
            self.buffer = buffer;
            return;
        }
        // GPUI's sprite atlas is BGRA; the backend hands us RGBA.
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let Some(buffer) =
            image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, buffer)
        else {
            return;
        };
        let new_image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        if let Some(old) = self.image.replace(new_image) {
            // Do not free the atlas texture immediately: blade may still have
            // the previous frame in flight, and destroying it can hang the
            // Intel driver. Keep a short history so the atlas is retired after
            // the GPU has moved on.
            self.old_images.push_back(old);
            while self.old_images.len() > 8 {
                if let Some(old) = self.old_images.pop_front() {
                    cx.drop_image(old, None);
                }
            }
        }
        self.last_frame = Some(Instant::now());
    }

    fn answer_credentials(&mut self, cx: &mut Context<Self>) {
        let password = self.password.read(cx).content();
        cx.global::<Passwords>()
            .set(&self.connection.address(), &password);
        if self.save_password
            && !password.is_empty()
            && let Err(e) = crate::secrets::store_password(&self.connection.address(), &password)
        {
            log::warn!("could not save the password: {e}");
        }
        self.backend.answer_credentials(Some(Credentials {
            user: self.connection.username.clone(),
            domain: String::new(),
            password,
        }));
        self.prompt = None;
        cx.notify();
    }

    fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        match self.prompt.take() {
            Some(Prompt::Credentials) => self.backend.answer_credentials(None),
            Some(_) => self.backend.answer_trust(false),
            None => {}
        }
        cx.notify();
    }

    fn accept_prompt(&mut self, cx: &mut Context<Self>) {
        match self.prompt.take() {
            Some(Prompt::Credentials) => self.answer_credentials(cx),
            Some(Prompt::Message {
                question: false, ..
            })
            | Some(Prompt::Trust { .. }) => {
                self.backend.answer_trust(true);
            }
            Some(Prompt::Message { .. }) => self.backend.answer_trust(true),
            None => {}
        }
        cx.notify();
    }

    /// Where the desktop is drawn, in window coordinates: the scale (logical
    /// pixels per desktop pixel), its origin, and its size. Painting and
    /// pointer mapping both go through this, so they cannot disagree.
    fn image_layout(&self, window: &Window) -> Option<ImageLayout> {
        let size = window.bounds().size;
        layout_for(
            self.scale,
            (self.facts.width as f32, self.facts.height as f32),
            (size.width.into(), size.height.into()),
            window.scale_factor(),
        )
    }

    fn remote_point(
        &self,
        window: &Window,
        position: gpui::Point<gpui::Pixels>,
    ) -> Option<(i32, i32)> {
        let layout = self.image_layout(window)?;
        let desktop_w = self.facts.width as f32;
        let desktop_h = self.facts.height as f32;
        let x = ((f32::from(position.x) - layout.x) / layout.scale).clamp(0.0, desktop_w - 1.0);
        let y = ((f32::from(position.y) - layout.y) / layout.scale).clamp(0.0, desktop_h - 1.0);
        Some((x as i32, y as i32))
    }

    fn send_pointer(&mut self, window: &Window, position: gpui::Point<gpui::Pixels>, mask: i32) {
        if let Some((x, y)) = self.remote_point(window, position) {
            self.pointer = (x, y);
            self.backend.pointer(x, y, mask);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window) {
        self.send_pointer(window, event.position, self.buttons);
    }

    fn on_mouse_down(&mut self, button: MouseButton, event: &MouseDownEvent, window: &mut Window) {
        self.buttons |= mouse_mask(button);
        self.send_pointer(window, event.position, self.buttons);
    }

    fn on_mouse_up(&mut self, button: MouseButton, event: &MouseUpEvent, window: &mut Window) {
        self.buttons &= !mouse_mask(button);
        self.send_pointer(window, event.position, self.buttons);
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window) {
        let delta = match event.delta {
            ScrollDelta::Lines(delta) => delta.y,
            ScrollDelta::Pixels(delta) => f32::from(delta.y) / 40.0,
        };
        if delta == 0.0 {
            return;
        }
        let wheel_mask = if delta > 0.0 { 16 } else { 8 };
        let steps = delta.abs().ceil().clamp(1.0, 5.0) as usize;
        if let Some((x, y)) = self.remote_point(window, event.position) {
            for _ in 0..steps {
                self.backend.pointer(x, y, self.buttons | wheel_mask);
                self.backend.pointer(x, y, self.buttons);
            }
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // While a prompt is open, Enter answers it and Escape cancels it.
        // These must not be forwarded to the far end.
        if self.prompt.is_some() {
            match event.keystroke.key.as_str() {
                "enter" | "return" => {
                    self.accept_prompt(cx);
                    return;
                }
                "escape" => {
                    self.cancel_prompt(cx);
                    return;
                }
                _ => {}
            }
        }
        // Ctrl+V offers the local clipboard before the key goes to the far
        // end, so a paste there sees the latest local text.
        if self.send_clipboard
            && event.keystroke.modifiers.control
            && !event.keystroke.modifiers.alt
            && event.keystroke.key == "v"
        {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.backend.clipboard(&text);
            }
        }
        // A local shortcut, not sent to the far end: the panel is this
        // client's chrome. With Grab keyboard on, it is forwarded like any
        // other key.
        if !self.grab_keyboard
            && event.keystroke.modifiers.control
            && !event.keystroke.modifiers.alt
            && event.keystroke.key == "i"
        {
            self.info_open = !self.info_open;
            cx.notify();
            return;
        }
        let modifiers = keys::modifiers(&event.keystroke);
        for (id, keysym) in &modifiers {
            self.backend.key_down(*keysym, *id);
        }
        if let Some(keysym) = keys::keysym(&event.keystroke) {
            let id = keys::key_id(&event.keystroke.key);
            self.backend.key_down(keysym, id);
            self.synthetic_modifiers.insert(id, modifiers);
        }
    }

    fn on_key_up(&mut self, event: &KeyUpEvent) {
        let id = keys::key_id(&event.keystroke.key);
        self.backend.key_up(id);
        if let Some(modifiers) = self.synthetic_modifiers.remove(&id) {
            for (modifier_id, _) in modifiers {
                self.backend.key_up(modifier_id);
            }
        }
    }

    fn status_text(&self) -> &'static str {
        match self.state {
            State::Connecting => "Connecting",
            State::Connected => "Connected",
            State::Closed { .. } => "Disconnected",
        }
    }

    fn status_color(&self) -> gpui::Rgba {
        match self.state {
            State::Connecting => theme::yellow(),
            State::Connected => theme::green(),
            State::Closed { .. } => theme::red(),
        }
    }
}

impl Drop for SessionWindow {
    fn drop(&mut self) {
        self.backend.disconnect();
    }
}

impl Render for SessionWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The password prompt takes the keyboard as soon as it appears, so
        // typing and then Enter work without touching the mouse.
        if self.focus_password && matches!(self.prompt, Some(Prompt::Credentials)) {
            self.focus_password = false;
            window.focus(&self.password.read(cx).handle());
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x000000))
            .font_family(theme::FONT_FAMILY)
            .text_color(theme::text())
            .child(self.titlebar())
            .child(
                div()
                    .id("session-canvas")
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .bg(rgb(0x050506))
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|this, event, _, cx| this.on_key_down(event, cx)))
                    .on_key_up(cx.listener(|this, event, _, _cx| this.on_key_up(event)))
                    .on_mouse_move(
                        cx.listener(|this, event, window, _cx| this.on_mouse_move(event, window)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_down(MouseButton::Left, event, window)
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_down(MouseButton::Middle, event, window)
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_down(MouseButton::Right, event, window)
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_up(MouseButton::Left, event, window)
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Middle,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_up(MouseButton::Middle, event, window)
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(|this, event, window, _cx| {
                            this.on_mouse_up(MouseButton::Right, event, window)
                        }),
                    )
                    .on_scroll_wheel(
                        cx.listener(|this, event, window, _cx| this.on_scroll(event, window)),
                    )
                    .child(match &self.state {
                        State::Connected => match (&self.image, self.image_layout(window)) {
                            (Some(image), Some(layout)) => div()
                                .absolute()
                                .left(px(layout.x))
                                .top(px(layout.y - 38.0))
                                .w(px(layout.width))
                                .h(px(layout.height))
                                .child(
                                    img(Arc::clone(image))
                                        .object_fit(ObjectFit::Fill)
                                        .size_full(),
                                )
                                .into_any_element(),
                            _ => centered_note("Waiting for the first frame…"),
                        },
                        State::Connecting => centered_note(
                            self.status
                                .as_deref()
                                .unwrap_or("Connecting to the remote desktop…"),
                        ),
                        State::Closed { detail } => self.closed_card(detail, cx),
                    })
                    .child(self.toolbar(cx))
                    .children(self.info_open.then(|| self.info_panel(cx)))
                    .children(self.prompt.as_ref().map(|_| self.prompt_overlay(cx))),
            )
    }
}

impl SessionWindow {
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
                    .id("session-title-drag")
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap(px(8.))
                    .h_full()
                    .pl(px(14.))
                    .min_w(px(0.))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(
                        div()
                            .flex_shrink_0()
                            .size(px(6.))
                            .rounded_full()
                            .bg(self.status_color()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .font_weight(theme::WEIGHT_MEDIUM)
                            .child(self.connection.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme::text_tertiary())
                            .child(self.connection.address()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(10.))
                            .text_color(theme::text_tertiary())
                            .child(self.status_text()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .pr(px(8.))
                    .child(session_window_button("−", 0))
                    .child(session_window_button("□", 1))
                    .child(session_window_button("✕", 2)),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        if matches!(self.state, State::Closed { .. }) {
            return div().into_any_element();
        }
        div()
            .absolute()
            .top(px(12.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .id("session-toolbar")
                    .occlude()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .h(px(38.))
                    .px(px(6.))
                    .rounded_full()
                    .bg(rgba(0x151518e6))
                    .border_1()
                    .border_color(rgba(0xffffff14))
                    .shadow_lg()
                    .child(toolbar_button("Auto", 0, self.scale == ScaleMode::Auto, cx))
                    .child(toolbar_button("Fit", 1, self.scale == ScaleMode::Fit, cx))
                    .child(toolbar_button(
                        "1:1",
                        5,
                        self.scale == ScaleMode::Actual,
                        cx,
                    ))
                    .child(divider())
                    .child(toolbar_button("Fullscreen", 2, false, cx))
                    .child(divider())
                    .child(toolbar_button("Info", 3, self.info_open, cx))
                    .child(toolbar_button("Disconnect", 4, false, cx)),
            )
            .into_any_element()
    }

    fn info_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut rows: Vec<(String, String)> = Vec::new();
        for fact in self.backend.info() {
            if !fact.diagnostic {
                rows.push((fact.label.to_string(), fact.value));
            }
        }
        rows.push((
            "Desktop".into(),
            format!("{} × {}", self.facts.width, self.facts.height),
        ));
        if !self.facts.monitors.is_empty() {
            rows.push(("Monitors".into(), self.facts.monitors.len().to_string()));
        }
        if let Some(traffic) = self.backend.traffic() {
            rows.push(("Received".into(), format_bytes(traffic.received)));
            if let Some(sent) = traffic.sent {
                rows.push(("Sent".into(), format_bytes(sent)));
            }
        }
        if let Some(at) = self.last_frame {
            rows.push((
                "Last frame".into(),
                format!("{:.0} ms ago", at.elapsed().as_secs_f64() * 1000.),
            ));
        }

        div()
            .id("session-info")
            .occlude()
            .absolute()
            .top(px(12.))
            .right(px(12.))
            .bottom(px(12.))
            .w(px(292.))
            .flex()
            .flex_col()
            .rounded_xl()
            .bg(rgba(0x1a1a1ef2))
            .border_1()
            .border_color(rgba(0xffffff14))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(theme::WEIGHT_SEMIBOLD)
                            .child("Session"),
                    )
                    .child(
                        div()
                            .id("info-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(22.))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::surface_hover()))
                            .text_size(px(12.))
                            .text_color(theme::text_secondary())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.info_open = false;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .child(
                div()
                    .id("info-rows")
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .px(px(16.))
                    .pb(px(14.))
                    .overflow_y_scroll()
                    .children(rows.into_iter().map(|(label, value)| {
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(theme::text_tertiary())
                                    .child(label),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(theme::text_secondary())
                                    .child(value),
                            )
                    })),
            )
            .into_any_element()
    }

    fn closed_card(&self, detail: &str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(theme::WEIGHT_SEMIBOLD)
                    .child(if detail.is_empty() {
                        "Disconnected"
                    } else {
                        "Connection failed"
                    }),
            )
            .children((!detail.is_empty()).then(|| {
                div()
                    .max_w(px(520.))
                    .px(px(16.))
                    .text_size(px(12.))
                    .text_color(theme::text_secondary())
                    .child(detail.to_string())
                    .into_any_element()
            }))
            .child(
                div()
                    .id("closed-close")
                    .mt(px(8.))
                    .flex()
                    .items_center()
                    .h(px(32.))
                    .px(px(16.))
                    .rounded_lg()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::border())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::surface_hover()))
                    .text_size(px(12.))
                    .text_color(theme::text_secondary())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.backend.disconnect();
                        window.remove_window();
                        cx.notify();
                    }))
                    .child("Close"),
            )
            .into_any_element()
    }

    fn prompt_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(prompt) = &self.prompt else {
            return div().into_any_element();
        };
        let (title, detail) = match prompt {
            Prompt::Credentials => ("Password required".to_string(), String::new()),
            Prompt::Trust { title, detail } => (title.clone(), detail.clone()),
            Prompt::Message { text, .. } => ("Message from the server".to_string(), text.clone()),
        };
        let is_credentials = matches!(prompt, Prompt::Credentials);
        let accept_label = match prompt {
            Prompt::Credentials => "Connect".to_string(),
            Prompt::Trust { .. } => "Trust".to_string(),
            Prompt::Message { confirm, .. } => confirm.clone(),
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::overlay_scrim())
            .child(
                div()
                    .occlude()
                    .flex()
                    .flex_col()
                    .w(px(440.))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(theme::surface_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .px(px(22.))
                            .pt(px(20.))
                            .pb(px(14.))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(theme::WEIGHT_SEMIBOLD)
                                    .child(title),
                            )
                            .children((!detail.is_empty()).then(|| {
                                div()
                                    .text_size(px(11.5))
                                    .text_color(theme::text_secondary())
                                    .child(detail)
                                    .into_any_element()
                            })),
                    )
                    .children(is_credentials.then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .px(px(22.))
                            .pb(px(16.))
                            .child(self.password.clone())
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme::text_tertiary())
                                    .child(
                                        "VNC password — no username needed. Press Enter to connect.",
                                    ),
                            )
                            .into_any_element()
                    }))
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
                            .child(
                                div()
                                    .id("prompt-cancel")
                                    .flex()
                                    .items_center()
                                    .h(px(32.))
                                    .px(px(14.))
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::surface_hover()))
                                    .text_size(px(12.5))
                                    .text_color(theme::text_secondary())
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("prompt-accept")
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
                                    .on_click(cx.listener(|this, _, _, cx| this.accept_prompt(cx)))
                                    .child(accept_label),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn session_window_button(glyph: &'static str, kind: u8) -> AnyElement {
    let close = kind == 2;
    div()
        .id(("session-window-button", kind as usize))
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
        .on_mouse_down(MouseButton::Left, move |_, window, _| match kind {
            0 => window.minimize_window(),
            1 => window.zoom_window(),
            _ => window.remove_window(),
        })
        .child(glyph)
        .into_any_element()
}

fn toolbar_button(
    label: &'static str,
    kind: u8,
    active: bool,
    cx: &mut Context<SessionWindow>,
) -> AnyElement {
    let danger = kind == 4;
    div()
        .id(("session-toolbar", kind as usize))
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(10.))
        .rounded_full()
        .cursor_pointer()
        .text_size(px(11.))
        .font_weight(theme::WEIGHT_MEDIUM)
        .text_color(if active {
            theme::accent()
        } else if danger {
            theme::text_secondary()
        } else {
            theme::text_secondary()
        })
        .when(active, |s| s.bg(theme::fade(theme::accent(), 0.14)))
        .hover(move |s| {
            if danger {
                s.bg(theme::fade(theme::red(), 0.18))
                    .text_color(theme::red())
            } else {
                s.bg(theme::surface_hover()).text_color(theme::text())
            }
        })
        .on_click(cx.listener(move |this, _, window, cx| match kind {
            0 => {
                this.scale = ScaleMode::Auto;
                cx.notify();
            }
            1 => {
                this.scale = ScaleMode::Fit;
                cx.notify();
            }
            5 => {
                this.scale = ScaleMode::Actual;
                cx.notify();
            }
            2 => window.toggle_fullscreen(),
            3 => {
                this.info_open = !this.info_open;
                cx.notify();
            }
            4 => {
                this.backend.disconnect();
                cx.notify();
            }
            _ => {}
        }))
        .child(label)
        .into_any_element()
}

fn divider() -> AnyElement {
    div()
        .w(px(1.))
        .h(px(16.))
        .mx(px(4.))
        .bg(rgba(0xffffff14))
        .into_any_element()
}

fn centered_note(text: &str) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        .text_color(theme::text_secondary())
        .child(text.to_string())
        .into_any_element()
}

fn layout_for(
    mode: ScaleMode,
    desktop: (f32, f32),
    window: (f32, f32),
    scale_factor: f32,
) -> Option<ImageLayout> {
    const TITLEBAR: f32 = 38.0;
    let (desktop_w, desktop_h) = desktop;
    if desktop_w <= 0. || desktop_h <= 0. {
        return None;
    }
    let content_w = window.0.max(1.0);
    let content_h = (window.1 - TITLEBAR).max(1.0);
    let fit = (content_w / desktop_w).min(content_h / desktop_h);
    let native = 1.0 / scale_factor.max(0.01);
    let scale = match mode {
        ScaleMode::Auto => fit.min(native),
        ScaleMode::Fit => fit,
        ScaleMode::Actual => native,
    };
    if scale <= 0.0 {
        return None;
    }
    Some(ImageLayout {
        scale,
        x: (content_w - desktop_w * scale) / 2.0,
        y: TITLEBAR + (content_h - desktop_h * scale) / 2.0,
        width: desktop_w * scale,
        height: desktop_h * scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn auto_stops_at_one_desktop_pixel_per_device_pixel() {
        let layout = layout_for(ScaleMode::Auto, (1024., 768.), (1580., 932.), 2.).unwrap();
        assert!(close(layout.scale, 0.5));
        assert!(close(layout.width, 512.));
        assert!(close(layout.height, 384.));
        assert!(close(layout.x, 534.));
        assert!(close(layout.y, 293.));
    }

    #[test]
    fn fit_fills_the_content_box_and_keeps_the_aspect_ratio() {
        let layout = layout_for(ScaleMode::Fit, (1024., 768.), (1580., 932.), 2.).unwrap();
        assert!(close(layout.scale, 1.1640625));
        assert!(close(layout.width, 1192.));
        assert!(close(layout.height, 894.));
        assert!(close(layout.x, 194.));
        assert!(close(layout.y, 38.));
    }

    #[test]
    fn actual_is_one_desktop_pixel_per_device_pixel() {
        let layout = layout_for(ScaleMode::Actual, (1024., 768.), (1580., 932.), 2.).unwrap();
        assert!(close(layout.scale, 0.5));
        assert!(close(layout.width, 512.));
        assert!(close(layout.height, 384.));
        assert!(close(layout.x, 534.));
        assert!(close(layout.y, 293.));
    }

    #[test]
    fn auto_scales_a_large_desktop_down_to_fit() {
        let layout = layout_for(ScaleMode::Auto, (3840., 2160.), (1580., 932.), 2.).unwrap();
        assert!(close(layout.scale, 1580. / 3840.));
        assert!(close(layout.width, 1580.));
        assert!(close(layout.x, 0.));
        assert!(close(layout.y, 40.625));
    }

    #[test]
    fn a_zero_sized_desktop_has_no_layout() {
        assert!(layout_for(ScaleMode::Fit, (0., 0.), (1580., 932.), 2.).is_none());
    }
}

fn mouse_mask(button: MouseButton) -> i32 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 4,
        MouseButton::Navigate(_) => 0,
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
