// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SESSION: &str = "session-12345678";
const SCHEMA: &str =
    include_str!("../../../../../Sugar/integrations/contracts/file-choice.schema.json");

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new(SESSION.into())
}

fn envelope(extra: Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut value = json!({"protocol": "sugarcubes.file-choice", "version": 1, "session": SESSION});
    if let (Some(target), Some(source)) = (value.as_object_mut(), extra.as_object()) {
        target.extend(source.clone());
    }
    serde_json::to_vec(&value)
}

fn request() -> Result<FileChoiceRequestName, FileChoiceError> {
    FileChoiceRequestName::new("open-1")
}

#[test]
fn every_answer_encodes_every_schema_field() -> TestResult {
    let encoded = |message: FileChoiceHostMessage| -> Result<Value, Box<dyn Error>> {
        Ok(serde_json::from_slice(&message.encode(&session()?)?)?)
    };
    let base = json!({"protocol": "sugarcubes.file-choice", "version": 1, "session": SESSION,
        "request": "open-1"});
    let with = |extra: Value| {
        let mut value = base.clone();
        if let (Some(target), Some(source)) = (value.as_object_mut(), extra.as_object()) {
            target.extend(source.clone());
        }
        value
    };
    assert_eq!(
        encoded(FileChoiceHostMessage::Chosen {
            request: request()?,
            name: "Demo.sugar".into(),
            relative: "Demo/Demo.sugar".into(),
        })?,
        with(json!({"type": "chosen", "name": "Demo.sugar", "relative": "Demo/Demo.sugar"}))
    );
    assert_eq!(
        encoded(FileChoiceHostMessage::Chosen {
            request: request()?,
            name: "copy.sugar".into(),
            relative: String::new(),
        })?,
        with(json!({"type": "chosen", "name": "copy.sugar", "relative": ""}))
    );
    assert_eq!(
        encoded(FileChoiceHostMessage::Dismissed { request: request()? })?,
        with(json!({"type": "dismissed"}))
    );
    for (reason, name) in
        [(FileChoiceFailure::Busy, "busy"), (FileChoiceFailure::Unavailable, "unavailable")]
    {
        assert_eq!(
            encoded(FileChoiceHostMessage::Failed { request: request()?, reason })?,
            with(json!({"type": "failed", "reason": name}))
        );
    }
    Ok(())
}

#[test]
fn names_and_paths_beyond_their_bounds_are_never_sent() -> TestResult {
    let chosen = |name: String, relative: String| -> Result<_, Box<dyn Error>> {
        Ok(FileChoiceHostMessage::Chosen { request: request()?, name, relative }
            .encode(&session()?))
    };
    assert!(chosen("a.sugar".into(), "a".repeat(MAX_FILE_CHOICE_PATH_UNITS))?.is_ok());
    for refused in [
        chosen("a".repeat(MAX_FILE_CHOICE_NAME_UNITS + 1), String::new())?,
        chosen("a.sugar".into(), "a".repeat(MAX_FILE_CHOICE_PATH_UNITS + 1))?,
    ] {
        assert!(matches!(refused, Err(FileChoiceError::Invalid)));
    }
    Ok(())
}

#[test]
fn the_page_asks_to_open_or_save_from_a_folder_with_one_extension() -> TestResult {
    let open = FileChoiceClientMessage::decode(&envelope(json!({"type": "open",
        "request": "open-1", "folder": "E:\\ComfyUI\\user\\default\\projects",
        "extension": "sugar"}))?)?;
    assert_eq!(open.session(), &session()?);
    assert_eq!(open.request.as_str(), "open-1");
    assert_eq!(open.folder, "E:\\ComfyUI\\user\\default\\projects");
    assert_eq!(open.extension.as_str(), "sugar");
    assert_eq!(open.purpose, FileChoicePurpose::Open);
    let save = FileChoiceClientMessage::decode(&envelope(json!({"type": "save",
        "request": "save-2", "folder": "/projects/Demo", "name": "Demo.sugar",
        "extension": "sugar"}))?)?;
    assert_eq!(save.purpose, FileChoicePurpose::Save { name: "Demo.sugar".into() });
    for refused in [
        json!({"type": "open", "request": "open-1", "folder": "/p"}),
        json!({"type": "open", "request": "open-1", "folder": "", "extension": "sugar"}),
        json!({"type": "open", "request": "Open 1", "folder": "/p", "extension": "sugar"}),
        json!({"type": "open", "request": "open-1", "folder": "/p", "extension": "*.sugar"}),
        json!({"type": "open", "request": "open-1", "folder": "/p", "extension": "su.gar"}),
        json!({"type": "open", "request": "open-1", "folder": "/p", "extension": "sugar",
            "path": "/etc/passwd"}),
        json!({"type": "save", "request": "save-2", "folder": "/p", "extension": "sugar"}),
        json!({"type": "chosen", "request": "open-1", "name": "a", "relative": ""}),
        json!({"type": "open", "request": "open-1", "folder": "/p", "extension": "sugar",
            "version": 2}),
    ] {
        assert!(FileChoiceClientMessage::decode(&envelope(refused.clone())?).is_err(), "{refused}");
    }
    assert!(matches!(
        FileChoiceClientMessage::decode(&vec![b' '; MAX_FILE_CHOICE_MESSAGE_BYTES + 1]),
        Err(FileChoiceError::TooLarge)
    ));
    Ok(())
}

#[test]
fn the_generated_shapes_identify_their_schema() -> TestResult {
    use sha2::{Digest, Sha256};
    let schema = SCHEMA.replace("\r\n", "\n");
    let source =
        include_str!("../../../../../Sugar/integrations/contracts/FileChoiceProtocol.generated.rs");
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated file choice schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}
