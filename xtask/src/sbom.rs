//! SPDX 2.3 software bill of materials for a release executable: generation and verification.

use std::fs;
use std::path::{self, Path};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::hash::sha256_file;
use crate::metadata::{self, Dependencies, Package};
use crate::pe::Architecture;
use crate::text::{now_utc_timestamp, percent_encode, sanitize_spdx_id};

pub struct Subject<'a> {
    pub package_name: &'a str,
    pub package_version: &'a str,
    pub architecture: Architecture,
    pub file_name: &'a str,
    pub artifact_sha256: &'a str,
    pub created: &'a str,
}

pub fn build_document(subject: &Subject, dependencies: &[Package]) -> Value {
    let name = subject.package_name;
    let version = subject.package_version;
    let architecture = subject.architecture.name();
    let hash = subject.artifact_sha256;
    let root_id = format!("SPDXRef-Package-{}", sanitize_spdx_id(name));
    let mut packages = vec![json!({
        "SPDXID": root_id,
        "name": name,
        "versionInfo": version,
        "downloadLocation": "NOASSERTION",
        "filesAnalyzed": true,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": "NOASSERTION",
        "copyrightText": "NOASSERTION",
        "checksums": [{ "algorithm": "SHA256", "checksumValue": hash }],
    })];
    let mut relationships = vec![json!({
        "spdxElementId": "SPDXRef-DOCUMENT",
        "relationshipType": "DESCRIBES",
        "relatedSpdxElement": root_id,
    })];

    let mut sorted: Vec<&Package> = dependencies.iter().collect();
    sorted.sort_by(|a, b| (&a.name, &a.version, &a.source).cmp(&(&b.name, &b.version, &b.source)));
    let mut index = 0;
    for dependency in sorted {
        if dependency.name == name && dependency.version == version {
            continue;
        }
        index += 1;
        let id = format!(
            "SPDXRef-Package-{index}-{}",
            sanitize_spdx_id(&dependency.name)
        );
        let license = dependency
            .license
            .as_deref()
            .filter(|license| !license.trim().is_empty())
            .unwrap_or("NOASSERTION");
        let download = dependency
            .source
            .as_deref()
            .filter(|source| !source.trim().is_empty())
            .unwrap_or("NOASSERTION");
        packages.push(json!({
            "SPDXID": id,
            "name": dependency.name,
            "versionInfo": dependency.version,
            "downloadLocation": download,
            "filesAnalyzed": false,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": license,
            "copyrightText": "NOASSERTION",
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": format!(
                    "pkg:cargo/{}@{}",
                    percent_encode(&dependency.name),
                    percent_encode(&dependency.version)
                ),
            }],
        }));
        relationships.push(json!({
            "spdxElementId": root_id,
            "relationshipType": "DEPENDS_ON",
            "relatedSpdxElement": id,
        }));
    }
    relationships.push(json!({
        "spdxElementId": root_id,
        "relationshipType": "CONTAINS",
        "relatedSpdxElement": "SPDXRef-File-WindowsArtifact",
    }));

    json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": format!("{name}-{version}-windows-{architecture}"),
        "documentNamespace": format!(
            "https://github.com/xt0n1-t3ch/{name}/sbom/{version}/windows-{architecture}/{hash}"
        ),
        "creationInfo": {
            "created": subject.created,
            "creators": ["Tool: codex-discord-presence-xtask-1"],
            "licenseListVersion": "3.26",
        },
        "packages": packages,
        "files": [{
            "SPDXID": "SPDXRef-File-WindowsArtifact",
            "fileName": subject.file_name,
            "checksums": [{ "algorithm": "SHA256", "checksumValue": hash }],
            "licenseConcluded": "NOASSERTION",
            "copyrightText": "NOASSERTION",
        }],
        "relationships": relationships,
    })
}

pub struct Request<'a> {
    pub root: &'a Path,
    pub artifact: &'a Path,
    pub output: &'a Path,
    pub package_name: &'a str,
    pub package_version: &'a str,
    pub architecture: Architecture,
}

pub fn generate(request: &Request) -> Result<String> {
    let artifact = path::absolute(request.artifact)?;
    let artifact_sha256 = sha256_file(&artifact)?;
    let metadata = metadata::load(request.root, Dependencies::Included)?;
    let file_name = artifact
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("'{}' has no file name", artifact.display()))?;
    let created = now_utc_timestamp();
    let document = build_document(
        &Subject {
            package_name: request.package_name,
            package_version: request.package_version,
            architecture: request.architecture,
            file_name: &file_name,
            artifact_sha256: &artifact_sha256,
            created: &created,
        },
        &metadata.packages,
    );
    let output = path::absolute(request.output)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating '{}'", parent.display()))?;
    }
    let mut text = serde_json::to_string_pretty(&document)?;
    text.push('\n');
    fs::write(&output, text).with_context(|| format!("writing '{}'", output.display()))?;
    let packages = document["packages"].as_array().map_or(0, Vec::len);
    Ok(format!(
        "SPDX SBOM generated: {} packages={packages} sha256={artifact_sha256}",
        output.display()
    ))
}

fn items<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

pub fn verify_document(
    document: &Value,
    artifact_file_name: &str,
    artifact_sha256: &str,
    package_name: &str,
    package_version: &str,
) -> Result<()> {
    if text(document, "spdxVersion") != Some("SPDX-2.3")
        || text(document, "dataLicense") != Some("CC0-1.0")
    {
        bail!("SBOM must be an SPDX 2.3 JSON document under CC0-1.0");
    }
    let packages = items(document, "packages");
    let roots = packages
        .iter()
        .filter(|package| {
            text(package, "name") == Some(package_name)
                && text(package, "versionInfo") == Some(package_version)
        })
        .count();
    if roots != 1 {
        bail!("SBOM must contain exactly one root package {package_name}@{package_version}");
    }
    let files: Vec<&Value> = items(document, "files")
        .iter()
        .filter(|file| text(file, "fileName") == Some(artifact_file_name))
        .collect();
    if files.len() != 1 {
        bail!("SBOM does not identify the Windows artifact");
    }
    let checksums: Vec<String> = items(files[0], "checksums")
        .iter()
        .filter(|checksum| {
            text(checksum, "algorithm")
                .is_some_and(|algorithm| algorithm.eq_ignore_ascii_case("SHA256"))
        })
        .filter_map(|checksum| text(checksum, "checksumValue"))
        .map(str::to_lowercase)
        .collect();
    if checksums.len() != 1 || checksums[0] != artifact_sha256 {
        bail!("SBOM Windows artifact checksum mismatch");
    }
    if packages.len() < 2 {
        bail!("SBOM dependency inventory is empty");
    }
    Ok(())
}

pub fn verify(
    artifact: &Path,
    sbom: &Path,
    package_name: &str,
    package_version: &str,
) -> Result<String> {
    let artifact = path::absolute(artifact)?;
    let sbom = path::absolute(sbom)?;
    let content =
        fs::read_to_string(&sbom).with_context(|| format!("reading '{}'", sbom.display()))?;
    let document: Value = serde_json::from_str(&content)
        .with_context(|| format!("'{}' is not valid JSON", sbom.display()))?;
    let file_name = artifact
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .with_context(|| format!("'{}' has no file name", artifact.display()))?;
    let sha256 = sha256_file(&artifact)?;
    verify_document(
        &document,
        &file_name,
        &sha256,
        package_name,
        package_version,
    )?;
    Ok(format!(
        "SPDX SBOM verified: package={package_name} version={package_version} sha256={sha256}"
    ))
}
