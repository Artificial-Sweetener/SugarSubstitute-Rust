// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the built library's producing-feed decoder, including production feature gating.
use sugarsubstitute_update::distribution::{Distribution, MAX_BYTES};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn configuration() -> serde_json::Value {
    serde_json::json!({"schema_version":1, "repository":vec![92;32], "feed": {
        "transport":"https", "base":"https://updates.example.test/releases/",
        "redirects":["https://assets.example.test/"]
    }})
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
    for field in ["schema", "repository", "unknown", "feed_unknown", "missing_channel_override"] {
        let mut value = configuration();
        match field {
            "schema" => value["schema_version"] = 2.into(),
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
