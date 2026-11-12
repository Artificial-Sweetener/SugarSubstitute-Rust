// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real worker claims and retained original input without creating a native process.
use super::*;
use delivery::NativeLaunchInput;

#[test]
fn launch_slot_claim_failure_keeps_absent_input_and_original_worker_charge()
-> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    crate::production::incident_host::tests::admitted_value(512 * 1024, |context| {
        let original = context.memory().claim(512 * 1024)?;
        let mut input = None;
        assert!(NativeLaunchInput::reserve(&context, &mut input).is_err());
        assert!(input.is_none());
        assert_eq!(original.bytes(), 512 * 1024);
        assert!(context.memory().claim(1).is_err());
        Ok(original)
    })
}

#[test]
fn launch_slot_repeated_reservation_keeps_same_original_input_under_full_charge()
-> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    crate::production::incident_host::tests::admitted_value(512 * 1024, |context| {
        let mut input = None;
        NativeLaunchInput::reserve(&context, &mut input)?;
        let original = input.as_ref().ok_or("Original native slot missing")?;
        let pointer = std::ptr::from_ref(original.input()) as usize;
        let remaining = context.memory().claim(512 * 1024 - delivery::NATIVE_INPUT_PEAK_BYTES)?;
        assert!(context.memory().claim(1).is_err());
        NativeLaunchInput::reserve(&context, &mut input)?;
        assert_eq!(
            std::ptr::from_ref(input.as_ref().ok_or("Repeated slot missing")?.input()) as usize,
            pointer
        );
        assert!(context.memory().claim(1).is_err());
        drop(remaining);
        Ok(input)
    })
}

struct RetainedSlotFailure {
    cause: std::io::Error,
    original: sugarsubstitute_execution::ChargedBlockingInput<Option<NativeLaunchInput>>,
    storage: Option<MemoryClaim>,
}
#[test]
fn launch_slot_closed_actual_admission_keeps_original_caller_input_unallocated() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake)?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut specification = spec()?;
    specification.reservation_bytes = 512 * 1024;
    let mut original = Some(None::<NativeLaunchInput>);
    scope.cancel();
    let rejection = client.submit_blocking_owned_box(
        &scope,
        specification,
        &mut original,
        |_original, _context| -> std::result::Result<(), ()> { Ok(()) },
    );
    assert!(matches!(rejection, Err(sugarsubstitute_execution::OwnedInputError::Admission(_))));
    assert!(matches!(original, Some(None)));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    assert!(matches!(original, Some(None)));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn launch_slot_actual_owned_admission_failure_keeps_whole_input_and_credit_after_join() -> Result<()>
{
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut specification = spec()?;
    specification.reservation_bytes = 512 * 1024;
    let mut original = Some(None);
    let mut job = client.submit_blocking_owned_box(
        &scope,
        specification,
        &mut original,
        |mut original, context| -> std::result::Result<(), RetainedSlotFailure> {
            let storage = context.memory().claim(
                512 * 1024
                    - size_of::<Option<NativeLaunchInput>>() as u64
                    - delivery::NATIVE_INPUT_PEAK_BYTES,
            );
            assert!(storage.is_ok());
            let cause = NativeLaunchInput::reserve(&context, original.input_mut());
            assert!(cause.is_ok());
            let command = WindowsLaunchCommand::from_utf16(
                &r"D:\owned-inert-launch-slot-fixture\reporter.exe"
                    .encode_utf16()
                    .collect::<Vec<_>>(),
                &r"D:\owned-inert-launch-slot-fixture".encode_utf16().collect::<Vec<_>>(),
                &[],
            );
            let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[]);
            assert!(command.is_ok());
            assert!(environment.is_ok());
            if let (Some(slot), Ok(command), Ok(environment)) =
                (original.input_mut().as_mut(), command, environment)
            {
                let native = WindowsLaunchInput::new(command, environment);
                assert!(native.is_ok());
                *slot.input_mut() = native.ok();
            }
            assert!(context.memory().claim(1).is_err());
            Err(RetainedSlotFailure {
                cause: std::io::Error::other("Original native-input producer failure"),
                original,
                storage: storage.ok(),
            })
        },
    )?;
    assert!(original.is_none());
    let actual = batch(&client, &wake)?;
    assert!(actual.notifications.iter().any(|notice| notice.scope_id == scope.id()
        && notice.job_id == specification.id
        && notice.kind == TerminalKind::Failure));
    let completion = job.try_take_completion().ok_or("Actual native-slot completion missing")?;
    assert_eq!(completion.spec, specification);
    let failure = match completion.outcome {
        JobOutcome::Failure(failure) => failure,
        _ => return Err("Original native-slot producer outcome changed".into()),
    };
    let pointer = std::ptr::from_ref(
        failure.original.input().as_ref().ok_or("Retained charged slot missing")?.input(),
    ) as usize;
    assert_eq!(failure.cause.to_string(), "Original native-input producer failure");
    drop(completion.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 512 * 1024);
    assert_eq!(
        std::ptr::from_ref(
            failure.original.input().as_ref().ok_or("Original slot lost after join")?.input(),
        ) as usize,
        pointer
    );
    let native = failure
        .original
        .input()
        .as_ref()
        .ok_or("Original slot missing")?
        .input()
        .as_ref()
        .ok_or("Original Windows input lost")?;
    assert!(native.required_reservation_bytes() > 131072);
    assert!(failure.storage.is_some());
    drop(failure);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
