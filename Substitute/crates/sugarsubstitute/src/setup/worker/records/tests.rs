// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the composed record session with actual native files and the production execution owner.
use super::*;
use crate::completion_wake::Wake;
use serde_json::json;
use sugarsubstitute_contracts::execution::{Clock, JobContext, ScopeId, ScopeSpec, WaitFuture};
use sugarsubstitute_execution::ExecutionOwner;

struct ClockZero;
impl Clock for ClockZero {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Test clocks replace time, while native work and reservation admission use the actual owner.
fn run(body: impl FnOnce(JobContext) -> io::Result<()> + Send + 'static) -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(ClockZero), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobSequence::new().allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(1_000_000_000)),
            reservation_bytes: 4 * 1024 * 1024,
            replacement: None,
        },
        body,
    )?;
    let completion = wake.completion(&client, &mut job)?;
    match completion.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause.into()),
        _ => Err("Record operation did not succeed".into()),
    }
}
fn request(
    session: &mut RecordSession,
    value: serde_json::Value,
    context: &JobContext,
) -> io::Result<Vec<serde_json::Value>> {
    session.perform(serde_json::from_value(value).map_err(io::Error::other)?, context)
}

#[test]
fn complete_native_session_preserves_full_bounds_original_revisions_and_stale_bytes() -> Result<()>
{
    run(|context| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("糖😀");
        let mut session = RecordSession::new(path.clone(), true);
        request(&mut session, json!({"op":"open","create":true}), &context)?;
        request(
            &mut session,
            json!({"op":"begin","child":null,"leaf":".substitute-managed.json","revision":null}),
            &context,
        )?;
        for offset in (0..65536).step_by(8192) {
            request(
                &mut session,
                json!({"op":"chunk","offset":offset,"hex":"ab".repeat(8192)}),
                &context,
            )?;
        }
        request(&mut session, json!({"op":"commit"}), &context)?;
        let frames = request(
            &mut session,
            json!({"op":"read","child":null,"leaf":".substitute-managed.json"}),
            &context,
        )?;
        assert_eq!(frames.len(), 8);
        assert!(frames.iter().all(|frame| frame["hex"] == "ab".repeat(8192)));
        assert!(request(&mut session, json!({"op":"begin","child":null,"leaf":".substitute-managed.json","revision":frames[0]["revision"]}), &context).is_err());
        let child = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        request(&mut session, json!({"op":"create_child","name":child}), &context)?;
        request(
            &mut session,
            json!({"op":"begin","child":child,"leaf":".substitute-provisioning.json","revision":null}),
            &context,
        )?;
        request(&mut session, json!({"op":"chunk","offset":0,"hex":"ab"}), &context)?;
        request(&mut session, json!({"op":"commit"}), &context)?;
        let frames = request(
            &mut session,
            json!({"op":"read","child":child,"leaf":".substitute-provisioning.json"}),
            &context,
        )?;
        let revision =
            frames[0]["revision"].as_u64().ok_or_else(|| io::Error::other("Revision absent"))?;
        request(
            &mut session,
            json!({"op":"begin","child":child,"leaf":".substitute-provisioning.json","revision":revision}),
            &context,
        )?;
        request(&mut session, json!({"op":"chunk","offset":0,"hex":"ff"}), &context)?;
        let record = path.join(child).join(".substitute-provisioning.json");
        std::fs::write(&record, b"changed")?;
        assert!(request(&mut session, json!({"op":"commit"}), &context).is_err());
        assert_eq!(std::fs::read(record)?, b"changed");
        Ok(())
    })
}
#[test]
fn incomplete_or_reordered_uploads_cannot_touch_filesystem_records() -> Result<()> {
    run(|context| {
        let temporary = tempfile::tempdir()?;
        let mut session = RecordSession::new(temporary.path().into(), true);
        request(&mut session, json!({"op":"open","create":false}), &context)?;
        request(
            &mut session,
            json!({"op":"begin","child":null,"leaf":".substitute-managed.json","revision":null}),
            &context,
        )?;
        assert!(
            request(&mut session, json!({"op":"chunk","offset":1,"hex":"ab"}), &context).is_err()
        );
        assert!(
            request(&mut session, json!({"op":"chunk","offset":0,"hex":"AB"}), &context).is_err()
        );
        request(&mut session, json!({"op":"chunk","offset":0,"hex":"ab"}), &context)?;
        assert!(
            request(&mut session, json!({"op":"chunk","offset":0,"hex":"ab"}), &context).is_err()
        );
        assert!(request(&mut session, json!({"op":"open","create":false}), &context).is_err());
        drop(session);
        assert!(std::fs::read_dir(temporary.path())?.next().is_none());
        Ok(())
    })
}
#[test]
fn inspect_sessions_cannot_create_or_gain_write_authority_from_messages() -> Result<()> {
    run(|context| {
        let temporary = tempfile::tempdir()?;
        let mut session = RecordSession::new(temporary.path().into(), false);
        assert!(request(&mut session, json!({"op":"open","create":true}), &context).is_err());
        request(&mut session, json!({"op":"open","create":false}), &context)?;
        assert!(request(&mut session, json!({"op":"begin","child":null,"leaf":".substitute-managed.json","revision":null}), &context).is_err());
        assert!(
            request(
                &mut session,
                json!({"op":"create_child","name":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}),
                &context
            )
            .is_err()
        );
        assert!(std::fs::read_dir(temporary.path())?.next().is_none());
        Ok(())
    })
}

#[test]
fn native_host_scope_rejects_foreign_replayed_and_malformed_storage_requests() -> Result<()> {
    let scope = RecordScope {
        request: "original".into(),
        path: PathBuf::new(),
        writable: false,
        serial: 7,
    };
    let valid = json!({"version":1,"id":"original","event":"storage","data":{"version":1,"serial":8,"operation":{"op":"open","create":false}}});
    assert!(scope.decode(&valid).is_ok());
    for (pointer, replacement) in [
        ("/id", json!("foreign")),
        ("/version", json!(2)),
        ("/data/version", json!(2)),
        ("/data/serial", json!(7)),
        ("/data/serial", json!(9)),
        ("/data/serial", json!(9_007_199_254_740_992_u64)),
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(pointer).ok_or("Fixture pointer absent")? = replacement;
        assert!(scope.decode(&invalid).is_err());
    }
    let mut invalid = valid;
    invalid["data"]["path"] = json!("E:/foreign");
    assert!(scope.decode(&invalid).is_err());
    Ok(())
}
