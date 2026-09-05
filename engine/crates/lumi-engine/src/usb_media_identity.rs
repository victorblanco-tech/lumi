//! Optional tiny media identity. Only the disposable USB worker calls this;
//! never the realtime engine, and never any Rekordbox-owned file.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub const FILE_NAME: &str = ".lumi-media.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaIdentity {
    pub schema_version: u32,
    pub media_id: String,
    pub source_id: String,
}

impl MediaIdentity {
    fn valid(&self) -> bool {
        self.schema_version == 1
            && self.media_id.len() == 36
            && self.media_id.chars().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    c == '-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
            && (self.source_id.starts_with("usb-fs:") || self.source_id.starts_with("usb-local:"))
            && (8..=200).contains(&self.source_id.len())
            && self
                .source_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b":-_".contains(&c))
    }
}

pub fn read(root: &Path) -> std::io::Result<Option<MediaIdentity>> {
    let path = root.join(FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(invalid());
    }
    let identity: MediaIdentity = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if !identity.valid() {
        return Err(invalid());
    }
    Ok(Some(identity))
}

pub fn register(root: &Path, source_id: &str, media_id: &str) -> std::io::Result<MediaIdentity> {
    if let Some(existing) = read(root)? {
        return Ok(existing);
    }
    let identity = MediaIdentity {
        schema_version: 1,
        media_id: media_id.to_owned(),
        source_id: source_id.to_owned(),
    };
    if !identity.valid() {
        return Err(invalid());
    }
    let bytes = serde_json::to_vec(&identity).map_err(|_| invalid())?;
    // Never replace an existing marker, including a partially written one.
    // Creation is exclusive; a failed write is visible and needs explicit repair.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(FILE_NAME))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(identity)
}

fn invalid() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "USB identity file is invalid or unsupported; it was not overwritten",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_survives_rename_and_never_overwrites_existing_marker()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("lumi-marker-{}", std::process::id()));
        fs::create_dir_all(&root)?;
        let id = "cda97db5-879c-4cef-a101-165668a78390";
        let first = register(&root, "usb-fs:v2-original", id)?;
        let second = register(
            &root,
            "usb-fs:v2-renamed",
            "03f87c85-a0fa-4677-9685-83b8bed2e2dc",
        )?;
        assert_eq!(first.source_id, second.source_id);
        assert_eq!(first.media_id, second.media_id);
        let path = root.join(FILE_NAME);
        fs::write(&path, b"{broken")?;
        assert!(register(&root, "usb-fs:v2-other", id).is_err());
        assert_eq!(fs::read(&path)?, b"{broken");
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
