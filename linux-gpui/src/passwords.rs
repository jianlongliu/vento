// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Passwords for this run of the app, keyed by address.
//!
//! The editor's password field and a session's password prompt write here;
//! connecting again in the same run can reuse the value without asking. This
//! is deliberately memory only: nothing is written to `connections.json` or
//! anywhere else.

use std::collections::HashMap;
use std::sync::Mutex;

use gpui::{App, Global};

#[derive(Default)]
pub struct Passwords(Mutex<HashMap<String, String>>);

impl Global for Passwords {}

impl Passwords {
    pub fn get(&self, address: &str) -> Option<String> {
        self.0.lock().unwrap().get(address).cloned()
    }

    pub fn set(&self, address: &str, password: &str) {
        if !password.is_empty() {
            self.0
                .lock()
                .unwrap()
                .insert(address.to_string(), password.to_string());
        }
    }

    pub fn forget(&self, address: &str) {
        self.0.lock().unwrap().remove(address);
    }
}

pub fn init(cx: &mut App) {
    if cx.try_global::<Passwords>().is_none() {
        cx.set_global(Passwords::default());
    }
}

#[cfg(test)]
mod tests {
    use super::Passwords;

    #[test]
    fn passwords_are_kept_for_this_run_only() {
        let passwords = Passwords::default();
        assert_eq!(passwords.get("192.0.2.1:5900"), None);
        passwords.set("192.0.2.1:5900", "secret");
        assert_eq!(passwords.get("192.0.2.1:5900").as_deref(), Some("secret"));
        passwords.set("192.0.2.1:5900", "");
        assert_eq!(
            passwords.get("192.0.2.1:5900").as_deref(),
            Some("secret"),
            "an empty answer must not erase a good password"
        );
        passwords.forget("192.0.2.1:5900");
        assert_eq!(passwords.get("192.0.2.1:5900"), None);
    }
}
