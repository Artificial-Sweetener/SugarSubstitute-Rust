// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resolve owned worker codes and explicitly preserve upstream-owned progress text.
use serde_json::{Value, json};
use std::error::Error;
use sugarsubstitute_contracts::localization::installer::{self, FailureMessage, ProgressMessage};

/// Require an owned code or the explicit upstream text owner, never an implicit raw-text route.
pub(super) fn progress(data: &mut Value, locale: &str) -> Result<(), Box<dyn Error>> {
    if !data["statusCode"].is_null() {
        let code: ProgressMessage = serde_json::from_value(data["statusCode"].clone())?;
        data["status"] = json!(code.text(installer::messages(locale)));
    } else if data["textOwner"] != "comfy-desktop" || !data["status"].is_string() {
        return Err("Worker progress has no supported text owner".into());
    }
    Ok(())
}

/// Replace developer evidence only after its independent diagnostic delivery.
pub(super) fn failure(
    data: &mut Value,
    locale: &str,
    cancelled: bool,
) -> Result<(), Box<dyn Error>> {
    let copy = installer::messages(locale);
    let message = if cancelled {
        copy.cancelled
    } else if data["statusCode"].is_null() {
        copy.setup_failed
    } else {
        serde_json::from_value::<FailureMessage>(data["statusCode"].clone())?.text(copy)
    };
    *data = json!({"message":message});
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_feedback_uses_the_current_locale_and_never_promotes_internal_causes()
    -> Result<(), Box<dyn Error>> {
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            let mut data = json!({"statusCode":"checkingPython", "status":"leaked English"});
            progress(&mut data, locale.id)?;
            assert_eq!(data["status"], locale.installer.checking_python);
            for (code, expected) in [
                (Value::Null, locale.installer.setup_failed),
                (json!("selectPython"), locale.installer.select_python),
                (json!("cacheFull"), locale.installer.cache_full),
            ] {
                let mut data = json!({"statusCode":code,"message":"private original cause"});
                let original = data.clone();
                failure(&mut data, locale.id, false)?;
                assert_eq!(data, json!({"message":expected}));
                assert_eq!(original["message"], "private original cause");
            }
            let mut data = json!({"message":"AbortError"});
            failure(&mut data, locale.id, true)?;
            assert_eq!(data["message"], locale.installer.cancelled);
        }
        Ok(())
    }
    #[test]
    fn upstream_copy_is_preserved_exactly_and_implicit_or_unknown_owned_routes_reject()
    -> Result<(), Box<dyn Error>> {
        let mut upstream = json!({"textOwner":"comfy-desktop", "status":"用户文本 😀 e\u{301}"});
        let original = upstream.clone();
        progress(&mut upstream, "ja")?;
        assert_eq!(original, upstream);
        for mut rejected in [
            json!({"status":"English product text"}),
            json!({"textOwner":"sugarsubstitute", "status":"English product text"}),
            json!({"statusCode":"setupFailed"}),
            json!({"statusCode":"invented"}),
        ] {
            assert!(progress(&mut rejected, "en").is_err());
        }
        assert!(failure(&mut json!({"statusCode":"checkingPython"}), "en", false).is_err());
        Ok(())
    }
}
