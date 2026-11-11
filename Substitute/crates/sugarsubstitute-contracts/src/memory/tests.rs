// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;

const SESSION: &str = "panel-0123456789abcdef";

fn decode(json: &str) -> Result<MemoryMessage, MemoryError> {
    MemoryMessage::decode(json.as_bytes())
}

#[test]
fn usage_reports_decode_with_their_consumers() -> Result<(), Box<dyn Error>> {
    let message = decode(&format!(
        r#"{{"protocol":"sugarcubes.memory","version":1,"session":"{SESSION}","type":"usage","usedBytes":3000,"demandBytes":9000,"consumers":[{{"id":"canvas.images","bytes":2000}},{{"id":"icons","bytes":1000}}]}}"#
    ))?;
    let MemoryMessage::Usage { session, used_bytes, demand_bytes, consumers } = message else {
        return Err("Usage decoded as another message".into());
    };
    assert_eq!(session.as_str(), SESSION);
    assert_eq!((used_bytes, demand_bytes), (3000, 9000));
    assert_eq!(consumers[0], MemoryConsumer { id: "canvas.images".into(), bytes: 2000 });
    Ok(())
}

#[test]
fn preferences_decode_as_shares_or_fixed_sizes() -> Result<(), Box<dyn Error>> {
    let share = decode(&format!(
        r#"{{"protocol":"sugarcubes.memory","version":1,"session":"{SESSION}","type":"preference","preference":{{"mode":"share","percent":40}}}}"#
    ))?;
    assert!(matches!(
        share,
        MemoryMessage::Preference { preference: MemoryPreferenceChoice::Share { percent: 40 }, .. }
    ));
    let fixed = decode(&format!(
        r#"{{"protocol":"sugarcubes.memory","version":1,"session":"{SESSION}","type":"preference","preference":{{"mode":"fixed","bytes":8589934592}}}}"#
    ))?;
    assert!(matches!(
        fixed,
        MemoryMessage::Preference {
            preference: MemoryPreferenceChoice::Fixed { bytes: 8_589_934_592 },
            ..
        }
    ));
    Ok(())
}

#[test]
fn foreign_extended_and_out_of_bounds_messages_are_rejected() {
    let base = |body: &str| {
        format!(r#"{{"protocol":"sugarcubes.memory","version":1,"session":"{SESSION}",{body}}}"#)
    };
    for json in [
        base(r#""type":"preference","preference":{"mode":"share","percent":9}}"#),
        base(r#""type":"preference","preference":{"mode":"share","percent":76}}"#),
        base(r#""type":"preference","preference":{"mode":"fixed","bytes":0}}"#),
        base(r#""type":"preference","preference":{"mode":"share","percent":25,"extra":1}}"#),
        base(r#""type":"usage","usedBytes":1,"demandBytes":1,"consumers":[{"id":"Upper","bytes":1}]}"#),
        base(r#""type":"usage","usedBytes":9007199254740992,"demandBytes":1,"consumers":[]}"#),
        base(r#""type":"usage","usedBytes":1,"demandBytes":1,"consumers":[],"extra":true}"#),
        base(r#""type":"allowance","generation":1}"#),
        format!(r#"{{"protocol":"sugarcubes.panel","version":1,"session":"{SESSION}","type":"usage","usedBytes":1,"demandBytes":1,"consumers":[]}}"#),
        format!(r#"{{"protocol":"sugarcubes.memory","version":2,"session":"{SESSION}","type":"usage","usedBytes":1,"demandBytes":1,"consumers":[]}}"#),
        r#"{"protocol":"sugarcubes.memory","version":1,"session":"x","type":"usage","usedBytes":1,"demandBytes":1,"consumers":[]}"#.into(),
    ] {
        assert!(decode(&json).is_err(), "{json}");
    }
    let many = (0..17).map(|index| format!(r#"{{"id":"c{index}","bytes":1}}"#)).collect::<Vec<_>>();
    assert!(
        decode(&base(&format!(
            r#""type":"usage","usedBytes":1,"demandBytes":1,"consumers":[{}]"#,
            many.join(",")
        )))
        .is_err()
    );
    assert!(matches!(
        MemoryMessage::decode(&vec![b' '; MAX_MEMORY_MESSAGE_BYTES + 1]),
        Err(MemoryError::TooLarge)
    ));
}

#[test]
fn allowances_encode_the_exact_host_direction_shape() -> Result<(), Box<dyn Error>> {
    let session = PanelSession::new(SESSION.into())?;
    let allowance = MemoryAllowance {
        generation: 3,
        level: MemoryLevel::Elevated,
        allowance_bytes: 1024,
        ceiling_bytes: 4096,
        total_bytes: 8192,
        preference: MemoryPreferenceChoice::Share { percent: 25 },
    };
    let encoded: serde_json::Value = serde_json::from_slice(&allowance.encode(&session)?)?;
    assert_eq!(
        encoded,
        serde_json::json!({
            "protocol": "sugarcubes.memory", "version": 1, "session": SESSION, "type": "allowance",
            "generation": 3, "level": "elevated", "allowanceBytes": 1024, "ceilingBytes": 4096,
            "totalBytes": 8192, "preference": {"mode": "share", "percent": 25}
        })
    );
    let inexact = MemoryAllowance { total_bytes: u64::MAX, ..allowance };
    assert!(matches!(inexact.encode(&session), Err(MemoryError::Invalid)));
    Ok(())
}
