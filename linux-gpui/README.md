# Ventox

A native Wayland remote desktop client for VNC, RDP and SPICE, built on
Rust and [GPUI](https://gpui.rs). This is the GPUI frontend for the
[Vento](https://github.com/pgaskin/vento) backend sources; the original
egui client remains in `linux/`.

## Install

Arch Linux packages are attached to each release:

```sh
# download the package and SHA256SUMS from the release page
sha256sum -c SHA256SUMS
pkexec pacman -U ventox-<version>-1-x86_64.pkg.tar.zst
```

The package installs the binary at `/usr/bin/ventox`, its backend libraries
under `/usr/lib/ventox/lib`, and the desktop entry and icon for the
`dev.ventox.Ventox` app ID.

## Usage

```sh
ventox                                   # connection manager
ventox --connect 192.0.2.10:5900         # direct session (TigerVNC)
ventox --backend libvnc --connect host:5900
ventox --list-backends                   # backend versions
ventox --versions                        # app, toolkit and backend versions
ventox --help
```

Backends: `tigervnc`, `libvnc`, `rfb` (Rust VNC), `freerdp`, `ironrdp`,
`spice`. RustDesk is kept as a saved-connection type but is not wired up
yet.

In a session, `Ctrl+I` toggles the information panel unless **Grab keyboard**
is enabled for the connection. The password prompt takes the keyboard
automatically; Enter submits and Escape cancels.

## Configuration

| What | Where |
| --- | --- |
| Saved connections | `~/.local/share/vento/connections.json` |
| Settings (theme, new-connection defaults) | `~/.local/share/vento/settings.json` |
| Saved passwords | `~/.local/share/vento/secrets.json` (AES-256-GCM, key in `password.key`, both mode 0600) |
| Per-backend state | `~/.local/state/vento` |

Themes: **Auto** reads omarchy's matugen output at
`~/.local/state/omarchy/current/theme/colors.toml` and falls back to the
system light/dark preference. **System**, **Latte** and **Mocha** select the
Catppuccin palettes explicitly.

## Building

```sh
git submodule update --init --depth 1 third_party/ironrdp third_party/ryll
./patch.sh -n ironrdp ryll
./linux-gpui/scripts/fetch-backends.sh release
cd linux-gpui
cargo build --release --locked
```

The C backends (TigerVNC, LibVNC, FreeRDP) are loaded at run time from
`lib/` beside the binary; `scripts/fetch-backends.sh` installs the prebuilt
libraries from the upstream Vento release. `scripts/setup-backends.sh` does
the submodule, patch and library steps together.

## Design notes

- Wayland only. X11 is not a target of this frontend.
- The C backend libraries are redistributed as prebuilt binaries from the
  upstream Vento release rather than rebuilt here.
- Remote frames are uploaded through GPUI as CPU images; a DMA-BUF/GPU path
  is not implemented yet.
- **Grab keyboard** currently keeps the client's own shortcuts from
  stealing keys; a full Wayland shortcuts inhibitor (`zwp_keyboard_shortcuts_inhibit_v1`)
  still needs support in the GPUI platform layer.
- RustDesk needs a locally built FFmpeg/dav1d and is not wired up yet.
- Android is out of scope.

## Uninstall

```sh
pacman -Rns ventox
rm -rf ~/.local/share/vento ~/.local/state/vento
```

## License

GPL-3.0-or-later. Third-party components and their licenses are listed in
`THIRD_PARTY_LICENSES.md` and installed under
`/usr/share/licenses/ventox/`.
