// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accept only this producer's exact closed fields; any other text is discarded.
use super::*;

const LINE: &str = "2026-11-03T00:00:00.000000Z  INFO sugarsubstitute_desktop::bulk_channel_diagnostic: operation=\"bulk_channel_refused\" schema=1 reason=non_fetch_context context=7 method=GET origin=absent";

fn line(miss: Miss, context: i32, method: Method, origin: Origin) -> String {
    format!(
        "INFO {TARGET}{MARKER}schema=1 reason={miss} context={context} method={method} origin={origin}"
    )
}

#[test]
fn every_closed_value_round_trips_through_its_wire_name() {
    assert_eq!(
        Record::parse(LINE.as_bytes()),
        Some(Record {
            miss: Miss::NonFetchContext,
            context: 7,
            method: Method::Get,
            origin: Origin::Absent
        })
    );
    for miss in Miss::ALL {
        for method in Method::ALL {
            for origin in Origin::ALL {
                for context in [0, 3, 15, i32::MAX, -1, i32::MIN] {
                    let record = Record { miss, context, method, origin };
                    let text = line(miss, context, method, origin);
                    assert_eq!(Record::parse(text.as_bytes()), Some(record), "{text}");
                }
            }
        }
    }
}

#[test]
fn every_reason_survives_the_diagnostic_redaction_vocabulary() {
    // The routed formatter redacts values containing these fragments.
    for miss in Miss::ALL {
        for fragment in ["token", "password", "secret", "key", "path", "http"] {
            assert!(!miss.as_str().contains(fragment), "{miss}");
        }
    }
}

#[test]
fn methods_reduce_to_their_closed_class_without_recording_text() {
    assert_eq!(Method::classify(Some("GET")), Method::Get);
    assert_eq!(Method::classify(Some("POST")), Method::Post);
    assert_eq!(Method::classify(Some("HEAD")), Method::Head);
    assert_eq!(Method::classify(Some("OPTIONS")), Method::Options);
    assert_eq!(Method::classify(Some("get")), Method::Other);
    assert_eq!(Method::classify(Some("PUT")), Method::Other);
    assert_eq!(Method::classify(None), Method::Unreadable);
}

#[test]
fn noncanonical_unknown_and_extended_lines_are_rejected() {
    for (from, to) in [
        ("schema=1", "schema=2"),
        ("reason=non_fetch_context", "reason=other"),
        ("reason=non_fetch_context", "reason=\"non_fetch_context\""),
        ("context=7", "context=07"),
        ("context=7", "context=+7"),
        ("context=7", "context=-0"),
        ("context=7", "context=2147483648"),
        ("context=7", "context="),
        ("method=GET", "method=PUT"),
        ("origin=absent", "origin=http://127.0.0.1:8190"),
        ("sugarsubstitute_desktop::bulk_channel_diagnostic:", "other:"),
        ("operation=\"bulk_channel_refused\"", "operation=\"bulk_channel_other\""),
    ] {
        assert!(Record::parse(LINE.replace(from, to).as_bytes()).is_none(), "accepted {to}");
    }
    for suffix in [" origin=absent", " url=private", " message=anything"] {
        assert!(Record::parse(format!("{LINE}{suffix}").as_bytes()).is_none(), "{suffix}");
    }
    let prefixed = LINE.replace("INFO ", "INFO operation=\"x\" ");
    assert!(Record::parse(prefixed.as_bytes()).is_none());
    assert!(Record::parse(&[b'x'; 513]).is_none());
}
