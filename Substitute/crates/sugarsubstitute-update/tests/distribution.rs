// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the built library's producing-feed decoder, including production feature gating.
use sugarsubstitute_update::distribution::{Distribution, MAX_BYTES, UpdatePolicy};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn configuration() -> serde_json::Value {
    serde_json::json!({"schema_version":2, "update_policy":"beta_eager", "repository":vec![92;32], "feed": {
        "transport":"https", "base":"https://updates.example.test/releases/",
        "redirects":["https://assets.example.test/"]
    }})
}

#[test]
fn compiled_update_policy_is_decoded_with_the_feed() -> Result {
    let beta = Distribution::decode(&serde_json::to_vec(&configuration())?)?;
    assert_eq!(beta.update_policy(), UpdatePolicy::BetaEager);
    let mut value = configuration();
    value["update_policy"] = "standard".into();
    let standard = Distribution::decode(&serde_json::to_vec(&value)?)?;
    assert_eq!(standard.update_policy(), UpdatePolicy::Standard);
    Ok(())
}

#[test]
fn compiled_feed_keeps_repository_identity_and_exact_redirect_origins() -> Result {
    let (identity, policy) =
        Distribution::decode(&serde_json::to_vec(&configuration())?)?.into_parts();
    assert_eq!(identity, [92; 32]);
    let initial = policy.asset("timestamp.json")?;
    assert_eq!(initial.request_target(), "/releases/timestamp.json");
    let asset = policy.redirect(&initial, "https://assets.example.test/payload?ticket=fixture")?;
    assert!(policy.admits(&asset).is_ok());
    assert!(policy.redirect(&initial, "https://unlisted.example.test/payload").is_err());
    assert!(policy.redirect(&initial, "http://assets.example.test/payload").is_err());
    Ok(())
}

#[test]
fn malformed_schema_fields_and_empty_identity_never_disable_or_reset_updates() -> Result {
    for field in [
        "schema",
        "previous_schema",
        "repository",
        "unknown",
        "feed_unknown",
        "missing_channel_override",
        "missing_policy",
        "unknown_policy",
    ] {
        let mut value = configuration();
        match field {
            "schema" => value["schema_version"] = 3.into(),
            "previous_schema" => value["schema_version"] = 1.into(),
            "missing_policy" => {
                if let Some(fields) = value.as_object_mut() {
                    fields.remove("update_policy");
                }
            }
            "unknown_policy" => value["update_policy"] = "remote".into(),
            "repository" => value["repository"] = serde_json::json!(vec![0; 32]),
            "unknown" => value["installed_sequence"] = 900.into(),
            "feed_unknown" => value["feed"]["trusted_root"] = "remote".into(),
            _ => value["channel"] = "canary".into(),
        }
        assert!(Distribution::decode(&serde_json::to_vec(&value)?).is_err(), "{field}");
    }
    Ok(())
}

#[test]
fn feed_bounds_and_https_policy_reject_invalid_build_inputs() -> Result {
    assert!(Distribution::decode(b"").is_err());
    assert!(Distribution::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
    for base in [
        "http://127.0.0.1:8188/",
        "https://user:secret@example.test/",
        "https://example.test/?secret=1",
        "https://example.test/no-slash",
    ] {
        let mut value = configuration();
        value["feed"]["base"] = base.into();
        assert!(Distribution::decode(&serde_json::to_vec(&value)?).is_err());
    }
    let mut value = configuration();
    value["feed"]["redirects"] = serde_json::json!(vec!["https://example.test/"; 9]);
    assert!(Distribution::decode(&serde_json::to_vec(&value)?).is_err());
    Ok(())
}

#[test]
fn loopback_feed_exists_only_in_explicit_fixture_builds() -> Result {
    let mut value = configuration();
    value["feed"] = serde_json::json!({"transport":"loopback", "port":12345});
    let decoded = Distribution::decode(&serde_json::to_vec(&value)?);
    assert_eq!(decoded.is_ok(), cfg!(feature = "boundary-fixtures"));
    if let Ok(distribution) = decoded {
        let (_, policy) = distribution.into_parts();
        let endpoint = policy.asset("timestamp.json")?;
        assert_eq!(endpoint.host()?, "127.0.0.1");
        assert_eq!(endpoint.port()?, 12345);
        assert!(!endpoint.secure());
        assert!(policy.redirect(&endpoint, "http://127.0.0.1:12346/timestamp.json").is_err());
    }
    value["feed"]["port"] = 0.into();
    assert!(Distribution::decode(&serde_json::to_vec(&value)?).is_err());
    Ok(())
}

#[test]
fn release_notes_are_an_optional_https_template_filled_only_with_a_plain_version() -> Result {
    let without = Distribution::decode(&serde_json::to_vec(&configuration())?)?;
    assert_eq!(without.release_notes("2.4.1"), None);
    let mut value = configuration();
    value["release_notes"] =
        "https://github.example.test/owner/repo/releases/tag/v{version}".into();
    let with = Distribution::decode(&serde_json::to_vec(&value)?)?;
    assert_eq!(
        with.release_notes("2.4.1").map(String::from),
        Some("https://github.example.test/owner/repo/releases/tag/v2.4.1".to_owned())
    );
    for version in ["", "2.4.1/../../x", "2.4.1?x=1", "2.4.1#x", "2.4.1 beta", &"9".repeat(65)] {
        assert_eq!(with.release_notes(version), None, "{version:?}");
    }
    assert!(with.release_notes("2.4.1-beta.3+build.7").is_some());
    for template in [
        "http://example.test/{version}",
        "https://user:secret@example.test/{version}",
        "https://example.test/notes",
        "https://example.test/{version}/{version}",
        "file:///C:/notes/{version}",
        "not an address {version}",
    ] {
        let mut value = configuration();
        value["release_notes"] = template.into();
        assert!(Distribution::decode(&serde_json::to_vec(&value)?).is_err(), "{template}");
    }
    Ok(())
}
