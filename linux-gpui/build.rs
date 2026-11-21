// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Two things: the rpath that lets the C backends be loaded from `lib/`
//! beside the binary, and the versions `about` reports (the upstream Vento
//! backend sources and the GPUI release come from the manifests, the Rust
//! compiler from the toolchain doing the build).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib:$ORIGIN/../lib");
    println!("cargo:rerun-if-changed=../linux/Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    let vento =
        package_version(&manifest.join("../linux/Cargo.toml")).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=VENTO_BACKEND_VERSION={vento}");

    let gpui =
        lock_version(&manifest.join("Cargo.lock"), "gpui").unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=GPUI_VERSION={gpui}");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rust = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|line| {
            line.split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=VENTOX_RUSTC_VERSION={rust}");
}

fn package_version(path: &PathBuf) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[package]" {
            in_package = true;
            continue;
        }
        if in_package {
            if line.starts_with('[') {
                break;
            }
            if let Some(rest) = line.strip_prefix("version = ") {
                return Some(rest.trim_matches('"').to_string());
            }
        }
    }
    None
}

fn lock_version(path: &PathBuf, crate_name: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut found = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            found = false;
            continue;
        }
        if line == format!("name = \"{crate_name}\"") {
            found = true;
            continue;
        }
        if found && let Some(rest) = line.strip_prefix("version = ") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}
