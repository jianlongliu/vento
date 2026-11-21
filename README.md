# Ventox, the GPUI-based Linux client for Vento

> **Ventox is free software.** Source: https://github.com/jianlongliu/vento. If you paid for it, you were overcharged.
>
> **Ventox 是免费开源软件。** 源码：https://github.com/jianlongliu/vento。如果你为它付过钱，那就是被多收了。

A native Wayland remote desktop client for VNC, RDP and SPICE, written in Rust on [GPUI](https://gpui.rs). It provides a modern connection manager and session window while reusing the Vento backend implementations.

一个原生 Wayland 的 VNC、RDP、SPICE 远程桌面客户端，使用 Rust 与 [GPUI](https://gpui.rs) 编写；复用 Vento 的后端实现，提供现代的连接管理器和会话窗口。

The repository also contains the upstream Vento sources under `backends/`, the original egui client under `linux/`, and the Android sources under `app/`. The GPUI client lives in `linux-gpui/` and is the target of this fork's Linux releases.

仓库同时包含 `backends/` 下的 Vento 上游后端源码、`linux/` 下的原 egui 客户端，以及 `app/` 下的 Android 源码。GPUI 客户端位于 `linux-gpui/`，是这个 fork 的 Linux 发布目标。

## Project status / 项目状态

Ventox is an experimental VNC viewer for Arch Linux and a vibe-coding project. Many features have not been verified in real-world use; treat it as work in progress.

Ventox 是一个面向 Arch Linux 的实验性 VNC Viewer，也是一个 vibe coding 项目。大量功能尚未在真实环境中验证，请把它当作仍在进行中的作品。

## Install / 安装

Arch Linux packages are published on the releases page:

Arch Linux 安装包发布在 releases 页面：

```sh
sha256sum -c SHA256SUMS
pkexec pacman -U ventox-<version>-1-x86_64.pkg.tar.zst
```

The package installs:

安装后包含：

| Path / 路径 | Contents / 内容 |
| --- | --- |
| `/usr/bin/ventox` | Symlink to the application<br>应用入口的符号链接 |
| `/usr/lib/ventox/vento` | Application binary<br>应用主程序 |
| `/usr/lib/ventox/lib/` | TigerVNC, LibVNC and FreeRDP backend libraries<br>TigerVNC、LibVNC、FreeRDP 后端库 |
| `/usr/share/applications/dev.ventox.Ventox.desktop` | Launcher entry<br>启动器条目 |
| `/usr/share/icons/hicolor/512x512/apps/dev.ventox.Ventox.png` | Application icon<br>应用图标 |
| `/usr/share/licenses/ventox/` | License and third-party notices<br>许可证与第三方声明 |

## Usage / 用法

```sh
ventox                                   # connection manager / 连接管理器
ventox --connect 192.0.2.10:5900         # direct session, TigerVNC by default / 直接连接，默认 TigerVNC
ventox --backend libvnc --connect host:5900
ventox --list-backends                   # backend versions / 后端版本
ventox --versions                        # app, toolkit, compiler and backend versions / 应用、工具链、编译器与后端版本
ventox --help
```

Available backends:

可用后端：

| Family / 类型 | Backends / 后端 |
| --- | --- |
| VNC | TigerVNC, LibVNC, Rust VNC |
| RDP | FreeRDP, IronRDP |
| SPICE | SPICE |
| RustDesk | Saved-connection type only; not wired up<br>仅保留连接类型，尚未接入 |

The connection manager supports saved connections, Quick Connect, a right-click menu for Connect/Edit/Delete, and per-connection clipboard and keyboard options. Passwords can be saved with the connection; they are encrypted at rest.

连接管理器支持保存连接、快速连接、右键 Connect/Edit/Delete 菜单，以及每个连接独立的剪贴板和键盘选项。密码可以随连接保存，并以加密形式落盘。

In a session, `Ctrl+I` toggles the information panel. The password prompt takes focus automatically; Enter submits and Escape cancels.

会话中按 `Ctrl+I` 切换信息面板。密码提示会自动获得焦点；回车提交，Esc 取消。

## Configuration / 配置

| File / 文件 | Contents / 内容 |
| --- | --- |
| `~/.local/share/vento/connections.json` | Saved connections<br>保存的连接 |
| `~/.local/share/vento/settings.json` | Theme choice and new-connection defaults<br>主题选择与新连接默认选项 |
| `~/.local/share/vento/secrets.json` | AES-256-GCM encrypted passwords<br>AES-256-GCM 加密密码 |
| `~/.local/share/vento/password.key` | Local encryption key, mode 0600<br>本地加密密钥，权限 0600 |
| `~/.local/state/vento/` | Backend session state, including known hosts<br>后端会话状态，包括 known hosts |

`connections.json` records each connection's protocol, address, username, clipboard options, keyboard-grab option and whether its password is saved. Runtime fields such as the online indicator and last-used label are not persisted.

`connections.json` 记录每个连接的协议、地址、用户名、剪贴板选项、键盘抓取选项，以及是否保存密码。在线状态和最近使用时间属于运行时字段，不会落盘。

Passwords are stored separately from the connection list. `password.key` is generated locally on first use and is never included in the repository or package. Removing the key makes previously saved passwords undecryptable; the client then asks for the password again.

密码与连接列表分开存储。`password.key` 在首次使用时于本地生成，不会进入仓库或安装包。删除该密钥后，之前保存的密码将无法解密，客户端会重新询问密码。

### Themes / 主题

| Choice / 选择 | Source / 来源 |
| --- | --- |
| Auto | omarchy's matugen output at `~/.local/state/omarchy/current/theme/colors.toml`; falls back to the system light/dark preference<br>读取 omarchy 的 matugen 输出；不存在时跟随系统日夜 |
| System | System light/dark preference with Catppuccin<br>跟随系统日夜，使用 Catppuccin |
| Latte | Catppuccin Latte |
| Mocha | Catppuccin Mocha |

The theme and the default clipboard/keyboard options for new connections are changed in Settings.

主题和新连接的默认剪贴板/键盘选项在 Settings 中修改。

## Build from source / 从源码构建

```sh
git submodule update --init --depth 1 third_party/ironrdp third_party/ryll
./patch.sh -n ironrdp ryll
./linux-gpui/scripts/fetch-backends.sh release
cd linux-gpui
cargo build --release --locked
```

`scripts/setup-backends.sh` performs the submodule, patch and library steps together. The C backend libraries are loaded at run time from `lib/` beside the binary; the prebuilt libraries come from the upstream Vento release.

`scripts/setup-backends.sh` 会一次性完成子模块、补丁和库的准备工作。C 后端库在运行时从二进制旁的 `lib/` 加载；预编译库来自上游 Vento 发布。

Build requirements on Arch Linux: Rust, Cargo, Clang/libclang, pkgconf, freetype2, fontconfig, Wayland and libxkbcommon development files.

Arch Linux 构建依赖：Rust、Cargo、Clang/libclang、pkgconf、freetype2、fontconfig、Wayland 和 libxkbcommon 开发文件。

## Known limitations / 已知限制

- **Wayland only.** X11 is not a target of the GPUI client.

  仅支持 Wayland；X11 不是 GPUI 客户端的目标。

- **Remote frames are uploaded through GPUI as CPU images.** A DMA-BUF/GPU path is not implemented.

  远程帧通过 GPUI 以 CPU 图像上传；尚未实现 DMA-BUF/GPU 零拷贝路径。

- **Grab keyboard prevents the client's own shortcuts from intercepting keys.** Full Wayland shortcut inhibition (`zwp_keyboard_shortcuts_inhibit_v1`) is not implemented by the GPUI platform layer, so compositor shortcuts are not captured.

  抓取键盘只能阻止客户端自己的快捷键拦截按键；GPUI 平台层尚未实现完整的 Wayland 快捷键抑制协议，因此合成器的全局快捷键仍不会被捕获。

- **RustDesk requires a locally built FFmpeg/dav1d and is not wired up.**

  RustDesk 需要本地构建 FFmpeg/dav1d，目前尚未接入。

- **The Android app is not part of this fork's release process.**

  Android 应用不属于这个 fork 的发布流程。

## Uninstall / 卸载

```sh
pacman -Rns ventox
rm -rf ~/.local/share/vento ~/.local/state/vento
```

## License / 许可

GPL-3.0-or-later. The Ventox name and official branding are covered by `TRADEMARK.md`. Third-party components and their license texts are listed in `linux-gpui/THIRD_PARTY_LICENSES.md` and installed under `/usr/share/licenses/ventox/`.

GPL-3.0-or-later。Ventox 名称与官方标识的使用规则见 `TRADEMARK.md`。第三方组件及其许可证文本列在 `linux-gpui/THIRD_PARTY_LICENSES.md`，并安装到 `/usr/share/licenses/ventox/`。
