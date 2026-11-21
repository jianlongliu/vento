// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Saved connection passwords.
//!
//! They live in `$XDG_DATA_HOME/vento/secrets.json` as AES-256-GCM blobs,
//! keyed by address and encrypted with a random key in `password.key` beside
//! the file (both mode 0600). The key is local to this account: it stops a
//! copied `connections.json` or a backup from carrying readable passwords,
//! but it is not a hardware keystore. A missing or changed key only makes
//! decryption fail, in which case the client asks for the password again.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

use base64::Engine;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};

use crate::store;

fn key_path() -> PathBuf {
    store::data_dir().join("password.key")
}

fn secrets_path() -> PathBuf {
    store::data_dir().join("secrets.json")
}

pub fn store_password(address: &str, password: &str) -> io::Result<()> {
    let key = key()?;
    let blob = encrypt(&key, password.as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut map = read_map();
    map.insert(
        address.to_string(),
        base64::engine::general_purpose::STANDARD.encode(blob),
    );
    write_map(&map)
}

pub fn load_password(address: &str) -> Option<String> {
    if !key_path().is_file() || !secrets_path().is_file() {
        return None;
    }
    let key = key().ok()?;
    let map = read_map();
    let blob = base64::engine::general_purpose::STANDARD
        .decode(map.get(address)?)
        .ok()?;
    let plain = decrypt(&key, &blob).ok()?;
    String::from_utf8(plain).ok()
}

pub fn forget_password(address: &str) {
    let mut map = read_map();
    if map.remove(address).is_some() {
        let _ = write_map(&map);
    }
}

fn key() -> io::Result<[u8; 32]> {
    let path = key_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    match fs::read(&path) {
        Ok(bytes) if bytes.len() == 32 => Ok(bytes.try_into().unwrap()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "password.key is not 32 bytes",
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut key = [0u8; 32];
            SystemRandom::new()
                .fill(&mut key)
                .map_err(|_| io::Error::other("no randomness for password.key"))?;
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    file.write_all(&key)?;
                    Ok(key)
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let bytes = fs::read(&path)?;
                    if bytes.len() == 32 {
                        Ok(bytes.try_into().unwrap())
                    } else {
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "password.key is not 32 bytes",
                        ))
                    }
                }
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

fn read_map() -> HashMap<String, String> {
    let Ok(data) = fs::read(secrets_path()) else {
        return HashMap::new();
    };
    serde_json::from_slice(&data).unwrap_or_default()
}

fn write_map(map: &HashMap<String, String>) -> io::Result<()> {
    let path = secrets_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let data = serde_json::to_vec_pretty(map)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    use std::io::Write;
    file.write_all(&data)?;
    drop(file);
    fs::rename(&tmp, &path)?;
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    Ok(())
}

fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key).map_err(|_| "invalid password key".to_string())?,
    );
    let mut nonce_bytes = [0u8; 12];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| "no randomness".to_string())?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut body = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut body)
        .map_err(|_| "could not encrypt".to_string())?;
    let mut blob = nonce_bytes.to_vec();
    blob.extend_from_slice(&body);
    Ok(blob)
}

fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 12 {
        return Err("password blob is truncated".into());
    }
    let (nonce_bytes, body) = blob.split_at(12);
    let nonce = Nonce::try_assume_unique_for_key(nonce_bytes)
        .map_err(|_| "invalid password nonce".to_string())?;
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, key).map_err(|_| "invalid password key".to_string())?,
    );
    let mut body = body.to_vec();
    let plain = key
        .open_in_place(nonce, Aad::empty(), &mut body)
        .map_err(|_| "could not decrypt".to_string())?;
    Ok(plain.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_survives_encryption() {
        let key = [7u8; 32];
        let blob = encrypt(&key, b"secret").unwrap();
        assert_eq!(decrypt(&key, &blob).unwrap(), b"secret");
    }

    #[test]
    fn the_wrong_key_is_rejected() {
        let blob = encrypt(&[7u8; 32], b"secret").unwrap();
        assert!(decrypt(&[8u8; 32], &blob).is_err());
    }
}
