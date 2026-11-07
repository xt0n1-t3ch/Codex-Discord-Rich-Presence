//! Portable Executable header check for Windows release binaries.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{self, Path};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Architecture {
    #[value(name = "x64")]
    X64,
    #[value(name = "arm64")]
    Arm64,
}

impl Architecture {
    pub fn name(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        }
    }

    pub fn machine(self) -> u16 {
        match self {
            Self::X64 => 0x8664,
            Self::Arm64 => 0xAA64,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub artifact: String,
    pub architecture: &'static str,
    pub machine: String,
}

pub fn check(path: &Path, architecture: Architecture) -> Result<Report> {
    let artifact =
        path::absolute(path).with_context(|| format!("resolving '{}'", path.display()))?;
    let shown = artifact.display().to_string();
    let mut file =
        File::open(&artifact).with_context(|| format!("opening Windows artifact '{shown}'"))?;
    let length = file
        .metadata()
        .with_context(|| format!("reading '{shown}'"))?
        .len();
    if length < 64 {
        bail!("Windows artifact '{shown}' is too small to be a PE image.");
    }
    let mut dos_header = [0u8; 64];
    file.read_exact(&mut dos_header)
        .with_context(|| format!("reading '{shown}'"))?;
    if !dos_header.starts_with(b"MZ") {
        bail!("Windows artifact '{shown}' is missing the MZ signature.");
    }
    let pe_offset = u32::from_le_bytes([
        dos_header[0x3C],
        dos_header[0x3D],
        dos_header[0x3E],
        dos_header[0x3F],
    ]);
    if u64::from(pe_offset) > length - 6 {
        bail!("Windows artifact '{shown}' has an invalid PE header offset.");
    }
    file.seek(SeekFrom::Start(u64::from(pe_offset)))
        .with_context(|| format!("reading '{shown}'"))?;
    let mut pe_header = [0u8; 6];
    file.read_exact(&mut pe_header)
        .with_context(|| format!("reading '{shown}'"))?;
    if !pe_header.starts_with(b"PE\0\0") {
        bail!("Windows artifact '{shown}' is missing the PE signature.");
    }
    let machine = u16::from_le_bytes([pe_header[4], pe_header[5]]);
    let expected = architecture.machine();
    if machine != expected {
        bail!(
            "Windows artifact '{shown}' machine is 0x{machine:04X}; expected 0x{expected:04X} for {}.",
            architecture.name()
        );
    }
    Ok(Report {
        artifact: shown,
        architecture: architecture.name(),
        machine: format!("0x{expected:04X}"),
    })
}
