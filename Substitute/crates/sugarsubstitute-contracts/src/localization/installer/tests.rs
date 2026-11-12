// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify complete serialized web contracts, formatting and exact locale fallback.
use super::*;
use std::{collections::BTreeSet, error::Error};

/// Reject missing, additional, malformed or mismatched interpolation slots.
fn placeholders(text: &str) -> BTreeSet<&str> {
    let mut remaining = text;
    let mut names = BTreeSet::new();
    while let Some(start) = remaining.find('{') {
        assert!(!remaining[..start].contains('}'));
        remaining = &remaining[start + 1..];
        let end = remaining.find('}').unwrap_or(remaining.len());
        assert!(end < remaining.len());
        let name = &remaining[..end];
        assert!(!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric()));
        assert!(names.insert(name));
        remaining = &remaining[end + 1..];
    }
    assert!(!remaining.contains('}'));
    names
}

#[test]
fn every_locale_serializes_the_same_complete_placeholder_contract() -> Result<(), Box<dyn Error>> {
    let canonical = serde_json::to_value(messages("en"))?;
    let canonical = canonical.as_object().ok_or("Catalog must be an object")?;
    for locale in super::super::LOCALES {
        let serialized = serde_json::to_value(messages(locale.id))?;
        let labels = serialized.as_object().ok_or("Catalog must be an object")?;
        assert_eq!(labels.keys().collect::<Vec<_>>(), canonical.keys().collect::<Vec<_>>());
        for (key, value) in labels {
            let text = value.as_str().ok_or("Catalog labels must be text")?;
            assert!(!text.trim().is_empty() && !text.contains('\0'));
            assert_eq!(
                placeholders(text),
                placeholders(canonical[key].as_str().ok_or("Text absent")?)
            );
        }
        assert_eq!(placeholders(messages(locale.id).step), BTreeSet::from(["current", "total"]));
        assert_ne!(messages(locale.id).preparing_installation, messages(locale.id).progress_title);
    }
    assert!(std::ptr::eq(messages("unknown"), messages("en")));
    assert!(std::ptr::eq(messages("ES"), messages("en")));
    Ok(())
}

#[test]
fn worker_codes_reject_arbitrary_copy_and_cross_surface_codes() -> Result<(), Box<dyn Error>> {
    for code in ["invented", "CheckingPython", "setupFailed", "Checking the Python environment"] {
        assert!(serde_json::from_value::<ProgressMessage>(serde_json::json!(code)).is_err());
    }
    for code in ["checkingPython", "invented", "Internal error"] {
        assert!(serde_json::from_value::<FailureMessage>(serde_json::json!(code)).is_err());
    }
    let progress: ProgressMessage = serde_json::from_str("\"checkingPython\"")?;
    let failure: FailureMessage = serde_json::from_str("\"selectPython\"")?;
    for locale in super::super::LOCALES {
        assert_eq!(progress.text(locale.installer), locale.installer.checking_python);
        assert_eq!(failure.text(locale.installer), locale.installer.select_python);
    }
    Ok(())
}
