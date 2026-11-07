//! SHA-256 hashing and the release checksum manifest.

use std::fs::{self, File};
use std::io;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub const CHECKSUM_MANIFEST: &str = "SHA256SUMS.txt";

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening '{}'", path.display()))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).with_context(|| format!("reading '{}'", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn write_checksum_manifest(directory: &Path, names: &[String]) -> Result<std::path::PathBuf> {
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();
    let mut manifest = String::new();
    for name in sorted {
        let hash = sha256_file(&directory.join(name))?;
        manifest.push_str(&format!("{hash}  {name}\n"));
    }
    let path = directory.join(CHECKSUM_MANIFEST);
    fs::write(&path, manifest).with_context(|| format!("writing '{}'", path.display()))?;
    Ok(path)
}
