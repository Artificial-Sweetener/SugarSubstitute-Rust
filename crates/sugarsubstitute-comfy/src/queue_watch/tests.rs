// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
use super::*;

fn status(remaining: &str) -> String {
    format!(
        r#"{{"type":"status","data":{{"status":{{"exec_info":{{"queue_remaining":{remaining}}}}},"sid":"abc"}}}}"#
    )
}

#[test]
fn only_status_messages_state_the_queue() {
    assert_eq!(queue_activity(&status("0")), Some(QueueActivity::Idle));
    assert_eq!(queue_activity(&status("3")), Some(QueueActivity::Busy));
    for other in [
        r#"{"type":"executing","data":{"node":null,"prompt_id":"p"}}"#.to_owned(),
        r#"{"type":"status","data":{"status":{}}}"#.to_owned(),
        status("-1"),
        status("\"2\""),
        "not json".to_owned(),
    ] {
        assert_eq!(queue_activity(&other), None, "{other}");
    }
}

#[cfg(windows)]
#[test]
fn a_watch_reports_only_a_change_from_what_it_knew() {
    use sugarsubstitute_execution::loopback_websocket::Incoming;
    let busy = status("1");
    assert_eq!(watch::changed(&Incoming::Text(&busy), None), Some(QueueActivity::Busy));
    assert_eq!(watch::changed(&Incoming::Text(&busy), Some(QueueActivity::Busy)), None);
    assert_eq!(
        watch::changed(&Incoming::Text(&status("0")), Some(QueueActivity::Busy)),
        Some(QueueActivity::Idle)
    );
    assert_eq!(watch::changed(&Incoming::Discarded { bytes: 9, binary: true }, None), None);
}

#[cfg(windows)]
#[test]
fn the_endpoint_is_loopback_with_this_process_as_client() -> Result<(), Box<dyn std::error::Error>>
{
    use sugarsubstitute_contracts::configuration::LoopbackOrigin;
    let origin = LoopbackOrigin::parse("http://localhost:8188", 8188)?;
    use sugarsubstitute_execution::loopback_websocket::{LoopbackFamily, Target};
    let expected = Target::loopback(
        LoopbackFamily::Ipv4,
        8188,
        &format!("/ws?clientId=sugarsubstitute-queue-{}", std::process::id()),
    )?;
    assert_eq!(target(origin)?, expected);
    Ok(())
}
