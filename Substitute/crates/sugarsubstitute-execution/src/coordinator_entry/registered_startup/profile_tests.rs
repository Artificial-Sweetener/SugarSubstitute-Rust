// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep authenticated originals and the selected child profile across actual Core restart custody.
use super::*;
use crate::coordinator_entry::protocol;
fn selected() -> TestResult<std::path::PathBuf> {
    Ok(request()?.profile_directory().join("user").join("desktop-data").join("profile"))
}
#[test]
fn registered_selected_child_profile_keeps_original_admission_and_restart_body_separate()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let snapshot = request_snapshot(&rig, &boundary, &session)?;
    let original_bytes = snapshot.original_body()?.to_vec();
    let original_path =
        protocol::launch_payload::decode(&original_bytes)?.profile_directory().le_bytes().to_vec();
    let (source, memory) = registered_profile(&rig, snapshot, selected()?)?;
    assert_eq!(source.state()?.body.as_ref(), original_bytes);
    assert_eq!(source.view()?.profile_directory().le_bytes(), original_path);
    assert_ne!(source.state()?.child_body.as_ref(), original_bytes);
    let expected = source.state()?.child_body.to_vec();
    assert_ne!(
        protocol::launch_payload::decode(&expected)?.profile_directory().le_bytes(),
        original_path
    );
    assert_eq!(REGISTERED_STARTUP_BYTES, 2 * 65_536 + 4096);
    assert!(memory.claim(BYTES - REGISTERED_STARTUP_BYTES).is_ok());
    assert!(memory.claim(BYTES - REGISTERED_STARTUP_BYTES + 1).is_err());
    drop(memory);
    let previous = run()?;
    let run = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            client.restart_startup_snapshot(&retained, previous, run, original, &context)
        },
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(snapshot) = completion.outcome else {
        return Err("Actual selected-profile restart failed".into());
    };
    assert_eq!(snapshot.original_body()?, original_bytes);
    assert_eq!(snapshot.body()?, expected);
    assert_eq!(source.view()?.profile_directory().le_bytes(), original_path);
    assert!(snapshot.launch.is_none());
    assert!(snapshot.initial_effective.is_none());
    drop(snapshot);
    drop(completion.memory);
    drop(source);
    session.control.close();
    drop(rig.completion(&mut session.completion)?);
    Ok(())
}
#[test]
fn registered_invalid_selected_profile_preserves_original_and_actual_one_shot_credit() -> TestResult
{
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let mut startup = request_snapshot(&rig, &boundary, &session)?;
    let expected = startup.original_body()?.to_vec();
    let until = startup.launch.as_ref().ok_or("Original Entry missing")?.until();
    let client = rig.client.clone();
    let original = run()?;
    let profile = selected()?;
    let mut job =
        rig.client.submit_blocking(&rig.scope, projection_spec(2, until)?, move |context| {
            assert!(
                client
                    .register_startup_source(
                        &mut startup,
                        &context,
                        original,
                        std::path::Path::new("relative")
                    )
                    .is_err()
            );
            assert!(startup.initial_effective.is_none());
            assert_eq!(
                startup
                    .original_body()
                    .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?,
                expected
            );
            assert!(context.memory().claim(BYTES).is_ok());
            assert!(
                !lock(&client.core.state)
                    .jobs
                    .get(&context.job_id())
                    .is_some_and(|record| record.registered_startup_issued)
            );
            client.register_startup_source(&mut startup, &context, original, &profile)
        })?;
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    drop(completion);
    session.control.close();
    drop(rig.completion(&mut session.completion)?);
    Ok(())
}
