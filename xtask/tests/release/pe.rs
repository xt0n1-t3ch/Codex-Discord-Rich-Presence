//! `check-pe` behavior for both Windows architectures.

use crate::common::{TempDir, Xtask, pe_image, write};

fn check(path: &std::path::Path, architecture: &str) -> crate::common::Run {
    Xtask::new("check-pe")
        .flag("--artifact", path)
        .flag("--architecture", architecture)
        .run()
}

#[test]
fn both_windows_architectures_are_recognised() {
    let temp = TempDir::new("pe-ok");
    let x64 = temp.join("x64.exe");
    let arm64 = temp.join("arm64.exe");
    write(&x64, pe_image(0x8664));
    write(&arm64, pe_image(0xAA64));
    let report = check(&x64, "x64").succeeds();
    assert!(
        report.stdout.contains("\"machine\":\"0x8664\""),
        "{}",
        report.stdout
    );
    assert_eq!(report.json()["architecture"], "x64");
    let report = check(&arm64, "arm64").succeeds();
    assert!(
        report.stdout.contains("\"machine\":\"0xAA64\""),
        "{}",
        report.stdout
    );
}

#[test]
fn an_architecture_mismatch_fails_closed() {
    let temp = TempDir::new("pe-mismatch");
    let x64 = temp.join("x64.exe");
    write(&x64, pe_image(0x8664));
    check(&x64, "arm64").fails_with("expected 0xAA64");
    let arm64 = temp.join("arm64.exe");
    write(&arm64, pe_image(0xAA64));
    check(&arm64, "x64").fails_with("machine is 0xAA64; expected 0x8664");
}

#[test]
fn files_that_are_not_pe_images_are_rejected() {
    let temp = TempDir::new("pe-broken");
    let small = temp.join("small.exe");
    write(&small, vec![0u8; 10]);
    check(&small, "x64").fails_with("too small to be a PE image");

    let mut no_mz = pe_image(0x8664);
    no_mz[0] = b'X';
    let path = temp.join("no-mz.exe");
    write(&path, no_mz);
    check(&path, "x64").fails_with("missing the MZ signature");

    let mut far_offset = pe_image(0x8664);
    far_offset[0x3C..0x40].copy_from_slice(&1000u32.to_le_bytes());
    let path = temp.join("far.exe");
    write(&path, far_offset);
    check(&path, "x64").fails_with("invalid PE header offset");

    let mut no_pe = pe_image(0x8664);
    no_pe[64] = b'X';
    let path = temp.join("no-pe.exe");
    write(&path, no_pe);
    check(&path, "x64").fails_with("missing the PE signature");

    check(&temp.join("absent.exe"), "x64").fails_with("absent.exe");
}
