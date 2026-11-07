//! Semantic Versioning parsing and precedence.

use std::cmp::Ordering;

use xtask::semver::{Version, parse_release_tag};
use xtask::text::{
    is_commit_sha, is_repository_slug, is_variable_name, percent_encode, sanitize_spdx_id,
    utc_timestamp,
};

fn version(text: &str) -> Version {
    text.parse()
        .unwrap_or_else(|error| panic!("{text}: {error}"))
}

#[test]
fn precedence_follows_the_semver_specification_example() {
    let ordered = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
    ];
    for pair in ordered.windows(2) {
        assert_eq!(
            version(pair[0]).precedence(&version(pair[1])),
            Ordering::Less,
            "{} must precede {}",
            pair[0],
            pair[1]
        );
    }
    assert_eq!(
        version("1.10.0").precedence(&version("1.9.0")),
        Ordering::Greater
    );
    assert_eq!(
        version("1.0.0+build.1").precedence(&version("1.0.0+build.2")),
        Ordering::Equal
    );
}

#[test]
fn invalid_versions_are_rejected() {
    for text in [
        "",
        "1",
        "1.0",
        "1.0.0.0",
        "01.0.0",
        "1.00.0",
        "1.0.0-",
        "1.0.0-01",
        "1.0.0-a..b",
        "1.0.0+",
        "1.0.0+a+b",
        "v1.0.0",
        "1.0.0-rc_1",
        "a.b.c",
        "1.0.-1",
    ] {
        assert!(
            text.parse::<Version>().is_err(),
            "'{text}' must be rejected"
        );
    }
}

#[test]
fn valid_versions_keep_their_text() {
    for text in [
        "0.0.0",
        "1.11.3",
        "1.8.0-rc.1",
        "1.8.0+build.7",
        "1.0.0-0.3.7",
        "1.0.0-x-y-z.--+meta-1",
    ] {
        assert_eq!(version(text).to_string(), text);
    }
    assert!(version("1.8.0-rc.1").is_prerelease());
    assert!(!version("1.8.0+build.7").is_prerelease());
}

#[test]
fn release_tags_need_the_v_prefix() {
    assert_eq!(
        parse_release_tag("v1.7.2").expect("valid").to_string(),
        "1.7.2"
    );
    for tag in ["1.7.2", "V1.7.2", "release-1.7.2", "v1.7", "v"] {
        let error = parse_release_tag(tag).expect_err("invalid tag");
        assert!(
            error.to_string().contains("valid SemVer release tag"),
            "{tag}: {error}"
        );
    }
}

#[test]
fn text_validators_match_the_old_patterns() {
    assert!(is_repository_slug("xt0n1-t3ch/Codex-Discord-Rich-Presence"));
    for slug in [
        "",
        "owner",
        "owner/",
        "/name",
        "a/b/c",
        "owner/na me",
        "owner/na\nme",
    ] {
        assert!(!is_repository_slug(slug), "'{slug}'");
    }
    assert!(is_commit_sha(&"a".repeat(40)));
    assert!(!is_commit_sha(&"A".repeat(40)));
    assert!(!is_commit_sha(&"a".repeat(39)));
    assert!(is_variable_name("RELEASE_APPROVED_SHA"));
    for name in ["", "release", "1ABC", "A-B", "_A"] {
        assert!(!is_variable_name(name), "'{name}'");
    }
}

#[test]
fn percent_encoding_matches_uri_escape_data_string() {
    assert_eq!(percent_encode("v1.8.0+build.7"), "v1.8.0%2Bbuild.7");
    assert_eq!(percent_encode("a b/c~d_e-f.g"), "a%20b%2Fc~d_e-f.g");
    assert_eq!(percent_encode("é"), "%C3%A9");
}

#[test]
fn spdx_identifiers_keep_only_letters_digits_dots_and_hyphens() {
    assert_eq!(sanitize_spdx_id("serde_json"), "serde-json");
    assert_eq!(sanitize_spdx_id("a.b-c"), "a.b-c");
}

#[test]
fn timestamps_are_utc_iso_8601() {
    assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
    assert_eq!(utc_timestamp(1_700_000_000), "2023-11-14T22:13:20Z");
    assert_eq!(utc_timestamp(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(utc_timestamp(4_102_444_799), "2099-12-31T23:59:59Z");
}
