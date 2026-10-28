// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify output request targets without opening files or network connections.

use super::*;

fn reference(json: &str) -> Result<CanvasFileReference, Box<dyn Error>> {
    Ok(CanvasFileReference::decode(json.as_bytes())?)
}

#[test]
fn encodes_exact_unicode_query_values_without_accepting_a_destination() -> Result<(), Box<dyn Error>>
{
    let file =
        reference(r#"{"filename":"résumé &?#%.png","subfolder":"batch/🌈 &","type":"output"}"#)?;
    let target = ComfyViewTarget::new(&file)?;
    let query = target.as_str().strip_prefix("/view?").ok_or("Unexpected target")?;
    let values: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    assert_eq!(
        values,
        [
            ("filename".into(), "résumé &?#%.png".into()),
            ("subfolder".into(), "batch/🌈 &".into()),
            ("type".into(), "output".into()),
        ]
    );
    let temporary =
        reference(r#"{"filename":"image.png","subfolder":"project\\nested","type":"temp"}"#)?;
    let target = ComfyViewTarget::new(&temporary)?;
    let query = target.as_str().strip_prefix("/view?").ok_or("Unexpected target")?;
    let values: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    assert_eq!(
        values,
        [
            ("filename".into(), "image.png".into()),
            ("subfolder".into(), "project/nested".into()),
            ("type".into(), "temp".into()),
        ]
    );
    Ok(())
}

#[test]
fn rejects_traversal_absolute_paths_root_overrides_and_oversized_targets()
-> Result<(), Box<dyn Error>> {
    for json in [
        r#"{"filename":"../outside.png","subfolder":"","type":"output"}"#,
        r#"{"filename":"a..b.png","subfolder":"","type":"output"}"#,
        r#"{"filename":"image.png[input]","subfolder":"","type":"output"}"#,
        r#"{"filename":"C:\\image.png","subfolder":"","type":"output"}"#,
        r#"{"filename":"","subfolder":"","type":"output"}"#,
        r#"{"filename":"image.png","subfolder":"../other","type":"output"}"#,
        r#"{"filename":"image.png","subfolder":"/absolute","type":"output"}"#,
        r#"{"filename":"image.png","subfolder":"line\nfeed","type":"output"}"#,
    ] {
        assert!(ComfyViewTarget::new(&reference(json)?).is_err(), "accepted {json}");
    }
    let huge = format!(r#"{{"filename":"{}","subfolder":"","type":"output"}}"#, "🌈".repeat(2048));
    assert_eq!(ComfyViewTarget::new(&reference(&huge)?), Err(ComfyViewError::TooLarge));
    Ok(())
}
