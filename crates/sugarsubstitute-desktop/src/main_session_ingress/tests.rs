// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real native session decoder against independent source and allocation facts.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    desktop_session::wire::{SessionMessage, SessionOperation},
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
struct Fixture {
    bootstrap: SessionBootstrap,
    identity: MainIdentity,
    message: SessionMessage,
    source: MemoryLease,
    memory: MemoryLease,
}
impl Fixture {
    fn new() -> Result<Self> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../third_party/sugarcubes-session/session_vectors.json"
        ))?;
        let webview = vectors["currentFence"]["webview"].as_str().ok_or("Missing webview")?;
        let bootstrap = SessionBootstrap::new(
            serde_json::from_value(vectors["scope"].clone())?,
            LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
            webview.into(),
        )?;
        let message = serde_json::from_value(vectors["messages"][0]["value"].clone())?;
        let source = MemoryLease::new(32 * 1024 * 1024, Box::new(Release));
        let memory = source.claim(24 * 1024 * 1024)?.into_lease();
        Ok(Self {
            bootstrap,
            message,
            source,
            memory,
            identity: MainIdentity {
                run: RunKey {
                    owner: CoordinatorKey {
                        namespace: NamespaceId::new([1; 32]),
                        epoch: CoordinatorEpoch::new([2; 16])?,
                    },
                    instance: InstanceId::new(1)?,
                    generation: RunGeneration::new(1)?,
                },
                launch: LaunchId::new(1)?,
                navigation: Generation::new(1)?,
            },
        })
    }
    fn text(&self, message: &SessionMessage) -> Result<String> {
        Ok(serde_json::to_string(&serde_json::json!({"sessionTransport":1,
        "webview":self.bootstrap.webview(),"ticket":1,"message":message}))?)
    }
    fn decode(&self, source: &str, text: &str) -> Result<Option<Received>> {
        Ok(decode(&self.bootstrap, self.identity, source, text, &self.memory, &self.source)?)
    }
}
#[test]
fn sixty_four_kib_chunk_does_not_borrow_panel_quota_and_retains_original_native_lease() -> Result {
    let mut fixture = Fixture::new()?;
    fixture.message.operation = SessionOperation::Chunk {
        transfer_id: "00000000-0000-4000-8000-000000000001".into(),
        index: 0,
        text: "x".repeat(64 * 1024),
    };
    let text = fixture.text(&fixture.message)?;
    assert!(text.len() > 32768);
    let received =
        fixture.decode("http://127.0.0.1:8188/", &text)?.ok_or("Missing admitted envelope")?;
    let observation = received.observation.ok_or("Valid chunk refused")?;
    assert_eq!(observation.identity(), fixture.identity);
    let (_, message, lease, claim) = observation.into_parts();
    assert!(lease.same_reservation(&fixture.source));
    assert_eq!(message, fixture.message);
    assert!(fixture.memory.claim(24 * 1024 * 1024).is_err());
    drop(claim);
    assert!(fixture.memory.claim(24 * 1024 * 1024).is_ok());
    Ok(())
}
#[test]
fn actual_source_scope_origin_and_webview_are_independent_admission_fences() -> Result {
    let fixture = Fixture::new()?;
    let text = fixture.text(&fixture.message)?;
    assert!(fixture.decode("http://evil.test/", &text)?.is_none());
    for mutation in 0..3 {
        let mut message = fixture.message.clone();
        match mutation {
            0 => message.scope.profile = "other".into(),
            1 => message.origin = "http://localhost:8188".into(),
            _ => {
                if let SessionOperation::Ready { fence, .. } = &mut message.operation {
                    fence.webview = "00000000-0000-4000-8000-000000000001".into();
                }
            }
        }
        let received = fixture
            .decode("http://127.0.0.1:8188/", &fixture.text(&message)?)?
            .ok_or("Known ticket disappeared")?;
        assert!(received.observation.is_none());
    }
    Ok(())
}
#[test]
fn duplicate_nested_contract_fields_are_refused_without_json_value_normalization() -> Result {
    let fixture = Fixture::new()?;
    let text = fixture.text(&fixture.message)?;
    let duplicate =
        text.replace("\"profile\":\"real\"", "\"profile\":\"real\",\"profile\":\"other\"");
    assert_ne!(text, duplicate);
    assert!(
        fixture
            .decode("http://127.0.0.1:8188/", &duplicate)?
            .ok_or("Known ticket missing")?
            .observation
            .is_none()
    );
    let extra = text.replacen("{", "{\"unexpected\":1,", 1);
    assert!(fixture.decode("http://127.0.0.1:8188/", &extra)?.is_none());
    Ok(())
}
