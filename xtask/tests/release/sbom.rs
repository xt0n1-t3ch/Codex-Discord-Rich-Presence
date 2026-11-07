//! `sbom` and `check-sbom` behavior.

use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use xtask::metadata::Package;
use xtask::pe::Architecture;
use xtask::sbom::{self, Subject};

use crate::common::{TempDir, Xtask, generate_lockfile, read, write};

const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn package(name: &str, version: &str, license: Option<&str>, source: Option<&str>) -> Package {
    Package {
        name: name.to_string(),
        version: version.to_string(),
        source: source.map(str::to_string),
        license: license.map(str::to_string),
        manifest_path: PathBuf::from("Cargo.toml"),
    }
}

fn document() -> Value {
    let dependencies = [
        package(
            "serde_json",
            "1.0.154",
            Some("MIT OR Apache-2.0"),
            Some("registry+https://github.com/rust-lang/crates.io-index"),
        ),
        package("codex-discord-presence", "1.11.3", Some("MIT"), None),
        package(
            "anyhow",
            "1.0.104",
            None,
            Some("registry+https://github.com/rust-lang/crates.io-index"),
        ),
        package("odd+name", "1.8.0+build.7", Some("MIT"), None),
    ];
    sbom::build_document(
        &Subject {
            package_name: "codex-discord-presence",
            package_version: "1.11.3",
            architecture: Architecture::Arm64,
            file_name: "codex-discord-rich-presence-windows-arm64.exe",
            artifact_sha256: HASH,
            created: "2026-11-07T06:00:00Z",
        },
        &dependencies,
    )
}

#[test]
fn the_document_is_spdx_2_3_with_the_artifact_checksum() {
    let document = document();
    assert_eq!(document["spdxVersion"], "SPDX-2.3");
    assert_eq!(document["dataLicense"], "CC0-1.0");
    assert_eq!(document["SPDXID"], "SPDXRef-DOCUMENT");
    assert_eq!(
        document["name"],
        "codex-discord-presence-1.11.3-windows-arm64"
    );
    assert_eq!(document["creationInfo"]["created"], "2026-11-07T06:00:00Z");
    assert_eq!(
        document["files"][0]["fileName"],
        "codex-discord-rich-presence-windows-arm64.exe"
    );
    assert_eq!(
        document["files"][0]["checksums"][0],
        json!({ "algorithm": "SHA256", "checksumValue": HASH })
    );
    assert!(
        document["documentNamespace"]
            .as_str()
            .expect("namespace")
            .ends_with(&format!("/1.11.3/windows-arm64/{HASH}"))
    );
}

#[test]
fn dependencies_are_sorted_numbered_and_exclude_the_root_package() {
    let document = document();
    let packages = document["packages"].as_array().expect("packages");
    let names: Vec<&str> = packages
        .iter()
        .map(|package| package["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        ["codex-discord-presence", "anyhow", "odd+name", "serde_json"]
    );
    assert_eq!(
        packages[0]["SPDXID"],
        "SPDXRef-Package-codex-discord-presence"
    );
    assert_eq!(packages[1]["SPDXID"], "SPDXRef-Package-1-anyhow");
    assert_eq!(packages[2]["SPDXID"], "SPDXRef-Package-2-odd-name");
    assert_eq!(packages[1]["licenseDeclared"], "NOASSERTION");
    assert_eq!(packages[3]["licenseDeclared"], "MIT OR Apache-2.0");
    assert_eq!(packages[2]["downloadLocation"], "NOASSERTION");
    assert_eq!(
        packages[2]["externalRefs"][0]["referenceLocator"],
        "pkg:cargo/odd%2Bname@1.8.0%2Bbuild.7"
    );
    let relationships = document["relationships"].as_array().expect("relationships");
    assert_eq!(relationships.len(), 5);
    assert_eq!(relationships[0]["relationshipType"], "DESCRIBES");
    assert_eq!(relationships[4]["relationshipType"], "CONTAINS");
}

#[test]
fn a_consistent_document_verifies() {
    let file = "codex-discord-rich-presence-windows-arm64.exe";
    sbom::verify_document(&document(), file, HASH, "codex-discord-presence", "1.11.3")
        .expect("the document verifies");
}

#[test]
fn a_tampered_document_is_rejected() {
    let file = "codex-discord-rich-presence-windows-arm64.exe";
    let verify = |document: &Value, file: &str, hash: &str, version: &str| {
        sbom::verify_document(document, file, hash, "codex-discord-presence", version)
            .expect_err("the document must be rejected")
            .to_string()
    };
    assert!(verify(&document(), file, &"0".repeat(64), "1.11.3").contains("checksum mismatch"));
    assert!(
        verify(&document(), "other.exe", HASH, "1.11.3")
            .contains("does not identify the Windows artifact")
    );
    assert!(verify(&document(), file, HASH, "1.11.4").contains("exactly one root package"));

    let mut wrong_version = document();
    wrong_version["spdxVersion"] = json!("SPDX-2.2");
    assert!(verify(&wrong_version, file, HASH, "1.11.3").contains("SPDX 2.3 JSON document"));

    let mut wrong_license = document();
    wrong_license["dataLicense"] = json!("MIT");
    assert!(verify(&wrong_license, file, HASH, "1.11.3").contains("CC0-1.0"));

    let mut no_dependencies = document();
    no_dependencies["packages"]
        .as_array_mut()
        .expect("packages")
        .truncate(1);
    assert!(
        verify(&no_dependencies, file, HASH, "1.11.3").contains("dependency inventory is empty")
    );

    let mut two_checksums = document();
    let entry = two_checksums["files"][0]["checksums"][0].clone();
    two_checksums["files"][0]["checksums"]
        .as_array_mut()
        .expect("checksums")
        .push(entry);
    assert!(verify(&two_checksums, file, HASH, "1.11.3").contains("checksum mismatch"));
}

fn dependency_fixture(temp: &TempDir) -> PathBuf {
    let root = temp.join("fixture");
    write(
        &root.join("Cargo.toml"),
        "[package]\nname = \"codex-discord-presence\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\ndep-a = { path = \"dep-a\" }\n",
    );
    write(&root.join("src/main.rs"), "fn main() {}\n");
    write(
        &root.join("dep-a/Cargo.toml"),
        "[package]\nname = \"dep-a\"\nversion = \"0.3.0\"\nedition = \"2024\"\nlicense = \"MIT\"\n",
    );
    write(&root.join("dep-a/src/lib.rs"), "");
    generate_lockfile(&root);
    root
}

#[test]
fn a_generated_sbom_verifies_and_a_changed_binary_does_not() {
    let temp = TempDir::new("sbom-roundtrip");
    let root = dependency_fixture(&temp);
    let artifact = temp.join("codex-discord-rich-presence-windows-x64.exe");
    write(&artifact, "abc");
    let output = temp.join("nested/out.spdx.json");
    let generated = Xtask::new("sbom")
        .root(&root)
        .flag("--artifact", &artifact)
        .flag("--output", &output)
        .flag("--package-version", "1.0.0")
        .flag("--architecture", "x64")
        .run()
        .succeeds();
    assert!(
        generated.stdout.contains("packages=2"),
        "{}",
        generated.stdout
    );
    assert!(generated.stdout.contains(HASH), "{}", generated.stdout);

    let text = read(&output);
    assert!(text.ends_with('\n'));
    let document: Value = serde_json::from_str(&text).expect("the SBOM is JSON");
    assert_eq!(document["packages"][1]["name"], "dep-a");
    assert_eq!(document["packages"][1]["licenseDeclared"], "MIT");
    assert_eq!(
        document["packages"][1]["externalRefs"][0]["referenceLocator"],
        "pkg:cargo/dep-a@0.3.0"
    );
    assert_eq!(
        document["creationInfo"]["created"]
            .as_str()
            .expect("created")
            .len(),
        20
    );

    let verified = Xtask::new("check-sbom")
        .flag("--artifact", &artifact)
        .flag("--sbom", &output)
        .flag("--package-version", "1.0.0")
        .run()
        .succeeds();
    assert!(
        verified
            .stdout
            .contains("SPDX SBOM verified: package=codex-discord-presence version=1.0.0")
    );

    fs::write(&artifact, "changed").expect("rewritten");
    Xtask::new("check-sbom")
        .flag("--artifact", &artifact)
        .flag("--sbom", &output)
        .flag("--package-version", "1.0.0")
        .run()
        .fails_with("checksum mismatch");
}

#[test]
fn an_sbom_without_dependencies_is_refused() {
    let temp = TempDir::new("sbom-empty");
    let root = temp.join("lonely");
    write(
        &root.join("Cargo.toml"),
        "[package]\nname = \"codex-discord-presence\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
    );
    write(&root.join("src/main.rs"), "fn main() {}\n");
    generate_lockfile(&root);
    let artifact = temp.join("app.exe");
    write(&artifact, "abc");
    let output = temp.join("out.spdx.json");
    Xtask::new("sbom")
        .root(&root)
        .flag("--artifact", &artifact)
        .flag("--output", &output)
        .flag("--package-version", "1.0.0")
        .run()
        .succeeds();
    Xtask::new("check-sbom")
        .flag("--artifact", &artifact)
        .flag("--sbom", &output)
        .flag("--package-version", "1.0.0")
        .run()
        .fails_with("dependency inventory is empty");
}
