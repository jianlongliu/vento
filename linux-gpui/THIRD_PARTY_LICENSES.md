# Third-party components

Ventox is GPL-3.0-or-later. It reuses the Vento backend sources and loads
prebuilt backend libraries from the upstream Vento release, and it depends on
the Rust crates listed in `Cargo.lock`.

| Component | License | Source |
| --- | --- | --- |
| Vento backend sources | GPL-3.0-or-later | https://github.com/pgaskin/vento |
| TigerVNC | GPL-2.0-or-later | https://github.com/TigerVNC/tigervnc |
| LibVNCServer | GPL-2.0-or-later | https://github.com/LibVNC/libvncserver |
| FreeRDP | Apache-2.0 | https://github.com/FreeRDP/FreeRDP |
| IronRDP | Apache-2.0 OR MIT | https://github.com/Devolutions/IronRDP |
| ryll / SPICE protocol | Apache-2.0 | https://github.com/shakenfist/ryll |
| GPUI (Zed) | Apache-2.0 | https://github.com/zed-industries/zed |
| bindgen (vendored patch) | BSD-3-Clause | https://github.com/rust-lang/rust-bindgen |

The prebuilt `libvento_libvnc.so`, `libvento_tigervnc.so` and
`libvento_freerdp.so` come from the Vento v8 Linux release. The full license
texts collected for this package are in `packaging/arch/licenses/`; the
distribution package installs them under
`/usr/share/licenses/ventox/third-party/`.

Every other Rust dependency keeps the license declared by its crate in
`Cargo.lock`; consult the corresponding crate release for its full text.
