// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the private cache subprotocol before admitting a worker or selecting retired grants.
use super::*;

/// Only these authenticated worker messages affect native cache coordination.
#[derive(Debug, PartialEq)]
pub(super) enum CacheEvent {
    Ready,
    DownloadsFull,
    MetadataFull,
    Other,
}
/// Integration readiness envelope two makes either mismatched host/worker fail before commands.
/// Authentication, cache/record/download operations and ordinary responses keep envelope one;
/// their independent schemas do not change when model integration gains an admitted storage base.
pub(super) fn cache_event(value: &serde_json::Value) -> Result<CacheEvent> {
    let version = if value["event"] == "ready" { 2 } else { 1 };
    if value["version"] != version {
        return Err("Unsupported provisioning response".into());
    }
    match value["event"].as_str() {
        Some("ready")
            if value["data"]["cacheProtocol"] == 2
                && value["data"]["storageProtocol"] == 1
                && value["data"]["metadataProtocol"] == 1
                && value["data"]["downloadsProtocol"] == 1
                && value["data"]["textProtocol"] == 1
                && value["data"]["integrationProtocol"] == 2 =>
        {
            Ok(CacheEvent::Ready)
        }
        Some("ready") => {
            Err("Unsupported provisioning storage/cache/text/integration protocol".into())
        }
        Some("cache_pressure") => match value["data"]["namespace"].as_str() {
            Some("downloads") => Ok(CacheEvent::DownloadsFull),
            Some("metadata") => Ok(CacheEvent::MetadataFull),
            _ => Err("Unsupported provisioning cache namespace".into()),
        },
        _ => Ok(CacheEvent::Other),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn legacy_and_unknown_cache_protocols_are_rejected_before_worker_readiness() {
        let valid = json!({"cacheProtocol":2,"storageProtocol":1,"metadataProtocol":1,"downloadsProtocol":1,"textProtocol":1,"integrationProtocol":2});
        for (field, version) in [
            ("cacheProtocol", 2),
            ("storageProtocol", 1),
            ("metadataProtocol", 1),
            ("downloadsProtocol", 1),
            ("textProtocol", 1),
            ("integrationProtocol", 2),
        ] {
            let mut missing = valid.clone();
            assert!(missing.as_object_mut().is_some_and(|object| object.remove(field).is_some()));
            assert!(
                cache_event(&json!({"version":2,"event":"ready","data":missing})).is_err(),
                "accepted missing {field}"
            );
            for invalid in [
                json!(0),
                json!(version + 1),
                json!(version.to_string()),
                json!(false),
                json!(null),
            ] {
                let mut data = valid.clone();
                data[field] = invalid;
                assert!(
                    cache_event(&json!({"version":2,"event":"ready","data":data})).is_err(),
                    "accepted invalid {field}"
                );
            }
        }
        let mut legacy = valid;
        legacy["integrationProtocol"] = json!(1);
        assert!(cache_event(&json!({"version":2,"event":"ready","data":legacy})).is_err());
        legacy["integrationProtocol"] = json!(2);
        legacy["cacheProtocol"] = json!(1);
        assert!(cache_event(&json!({"version":2,"event":"ready","data":legacy})).is_err());
    }
    #[test]
    fn current_native_grant_protocol_admits_the_matching_worker() -> Result<()> {
        assert_eq!(
            cache_event(
                &json!({"version":2,"event":"ready","data":{"cacheProtocol":2,"storageProtocol":1,"metadataProtocol":1,"downloadsProtocol":1,"textProtocol":1,"integrationProtocol":2}})
            )?,
            CacheEvent::Ready
        );
        // Earlier hosts require envelope one, so this version-two readiness cannot accidentally
        // admit a new worker there. Conversely the current host refuses an old readiness envelope
        // even if a peer adds an integration capability without changing its contract.
        assert!(cache_event(&json!({"version":1,"event":"ready","data":{"cacheProtocol":2,"storageProtocol":1,"metadataProtocol":1,"downloadsProtocol":1,"textProtocol":1,"integrationProtocol":2}})).is_err());
        assert!(cache_event(&json!({"version":1,"event":"ready","data":{"cacheProtocol":2,"storageProtocol":1,"metadataProtocol":1,"downloadsProtocol":1,"textProtocol":1}})).is_err());
        Ok(())
    }
    #[test]
    fn pressure_can_select_only_known_disposable_worker_grants() -> Result<()> {
        for (namespace, expected) in
            [("downloads", CacheEvent::DownloadsFull), ("metadata", CacheEvent::MetadataFull)]
        {
            assert_eq!(
                cache_event(
                    &json!({"version":1,"event":"cache_pressure","data":{"namespace":namespace}})
                )?,
                expected
            );
        }
        for namespace in ["configuration", "state", "runtime", "", "../metadata"] {
            assert!(
                cache_event(
                    &json!({"version":1,"event":"cache_pressure","data":{"namespace":namespace}})
                )
                .is_err()
            );
        }
        Ok(())
    }
    #[test]
    fn malformed_frames_never_admit_readiness_or_cache_cleanup() {
        for value in [
            json!(null),
            json!("ready"),
            json!({"event":"ready","data":{"cacheProtocol":1,"storageProtocol":1,"metadataProtocol":1}}),
            json!({"version":1,"event":"cache_pressure","data":[]}),
        ] {
            assert!(cache_event(&value).is_err());
        }
    }
}
