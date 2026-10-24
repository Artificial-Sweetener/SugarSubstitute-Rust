// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Embed the original installer wordmark and a local, capability-free presentation document.
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::localization::{self, installer};

/// Compose capability-free presentation from canonical copy and an inert location suggestion.
/// # Errors
/// Rejects non-Unicode locations and malformed embedded assets before publishing HTML.
pub(super) fn html(locale: &str, install_root: &Path) -> Result<String, Box<dyn Error>> {
    let install_root = install_root.to_str().ok_or("Installer location is not valid Unicode")?;
    let locales: Vec<_> = localization::LOCALES
        .iter()
        .map(|locale| {
            serde_json::json!({
                "id": locale.id,
                "label": locale.name,
            })
        })
        .collect();
    let bootstrap = serde_json::to_string(&serde_json::json!({
        "messages": installer::messages(locale),
        "locales": locales, "locale": localization::messages(locale).id,
        "installRoot": install_root,
    }))?
    .replace('<', "\\u003c");
    let mark = include_str!("../../../../installer/surface/wordmark.svg");
    let svg = mark.find("<svg").ok_or("Installer wordmark SVG is absent")?;
    let mut document = include_str!("../../../../installer/surface/index.html")
        .replace("__STYLE__", include_str!("../../../../installer/surface/style.css"))
        .replace("__LOCALE__", localization::messages(locale).id)
        .replace("__TITLE__", installer::messages(locale).title)
        .replace("__WORDMARK__", &mark[svg..])
        .replace("__WORDMARK_DESCRIPTION__", installer::messages(locale).wordmark_description)
        .replace("__BOOTSTRAP__", &bootstrap)
        .replace("__SCRIPT__", include_str!("../../../../installer/surface/controller.js"));
    for (name, source) in [
        ("LANGUAGE", include_str!("../../../../installer/surface/icons/Language.svg")),
        ("FOLDER", include_str!("../../../../installer/surface/icons/Folder.svg")),
        ("HOME", include_str!("../../../../installer/surface/icons/Home.svg")),
        ("LINK", include_str!("../../../../installer/surface/icons/Link.svg")),
        ("SYNC", include_str!("../../../../installer/surface/icons/Sync.svg")),
        ("ACCEPT", include_str!("../../../../installer/surface/icons/Accept.svg")),
        ("CLOSE", include_str!("../../../../installer/surface/icons/Close.svg")),
        ("CHEVRON", include_str!("../../../../installer/surface/icons/ChevronDown.svg")),
    ] {
        let start = source.find("<svg").ok_or("Installer Fluent icon SVG is absent")?;
        document = document.replace(&format!("__ICON_{name}__"), &source[start..]);
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn malformed_native_location_returns_an_error_before_json_construction() {
        use std::os::windows::ffi::OsStringExt;
        let location =
            std::path::PathBuf::from(std::ffi::OsString::from_wide(&[69, 58, 92, 0xdfff]));
        let failure = html("en", &location).err().map(|cause| cause.to_string());
        assert_eq!(failure.as_deref(), Some("Installer location is not valid Unicode"));
    }
    #[test]
    fn original_identity_is_embedded_and_all_owned_copy_is_localized() -> Result<(), Box<dyn Error>>
    {
        for locale in localization::LOCALES {
            let html = html(locale.id, Path::new(r"E:\Substitute"))?;
            assert!(html.contains("<title id=\"title\">SugarSubstitute</title>"));
            assert!(html.contains("id=\"main-lettering\""));
            assert!(html.contains(&format!(
                "<desc id=\"desc\" data-text=\"wordmarkDescription\">{}</desc>",
                installer::messages(locale.id).wordmark_description,
            )));
            assert!(!html.contains("__WORDMARK_DESCRIPTION__"));
            assert!(!html.contains("__BOOTSTRAP__"));
            assert!(!html.contains("__WORDMARK__"));
            assert!(!html.contains("__ICON_"));
            assert!(html.contains("connect-src 'none'"));
            assert!(html.contains(installer::messages(locale.id).language_title));
            assert!(html.contains(&format!("<html lang=\"{}\">", locale.id)));
            assert!(html.contains(&format!("<title>{}</title>", locale.installer.title)));
            assert!(html.contains(locale.name));
        }
        Ok(())
    }
    #[test]
    fn bootstrap_paths_cannot_close_the_authored_script_element() -> Result<(), Box<dyn Error>> {
        let html = html("en", Path::new(r"E:\</script><script>bad"))?;
        assert!(!html.contains(r"E:\\</script>"));
        assert!(html.contains("\\u003c/script>"));
        Ok(())
    }
}
