// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify startup file consumers through genuine original execution admission.
use super::*;
#[test]
fn shared_input_keeps_original_open_cause_and_rejects_unfunded_bounds() -> TestResult {
    for oversized in [false, true] {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let directory = temporary_directory("shared-input")?;
        let path = directory.join("absent.toml");
        let original = EntrySetupBudget::new(Moment::from_nanos(0))?;
        let specification = JobSpec {
            id: JobSequence::new().allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original.until()),
            reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
            replacement: None,
        };
        let mut job = client
            .submit_entry_preparation(&scope, specification, original, move |context| {
                let maximum = if oversized {
                    CONFIGURATION_RESERVATION_BYTES as usize
                } else {
                    MAX_STARTUP_CONFIGURATION_BYTES
                };
                read_preparation_file(&context, &path, maximum).map_err(ConfigurationCause::read)
            })
            .map_err(|rejected| rejected.error)?;
        let completion = wake.completion(&client, &mut job)?;
        let memory = completion.memory.clone();
        let JobOutcome::Failure(failure) = completion.outcome else {
            return Err("Expected original input failure".into());
        };
        match failure.cause() {
            EntryPreparationCause::Factory(ConfigurationCause::TooLarge) if oversized => {}
            EntryPreparationCause::Factory(ConfigurationCause::Read { source, after: None })
                if !oversized =>
            {
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound)
            }
            _ => return Err("Original input failure was replaced".into()),
        }
        assert!(memory.claim(1).is_err());
        drop(failure);
        assert_eq!(
            memory.claim(CONFIGURATION_RESERVATION_BYTES)?.bytes(),
            CONFIGURATION_RESERVATION_BYTES
        );
        std::fs::remove_dir(directory)?;
    }
    Ok(())
}

#[test]
fn startup_file_over_bound_rejects_before_schema_parsing() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let directory = temporary_directory("input-bound")?;
    let mut paths = options()?.into_paths();
    paths[0] = directory.join("oversized.toml");
    let input = StartupPaths::new(paths)?;
    std::fs::write(input.configuration(), vec![b'x'; MAX_STARTUP_CONFIGURATION_BYTES + 1])?;
    let original = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut job = client
        .submit_entry_preparation(
            &scope,
            JobSpec {
                id: JobSequence::new().allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(original.until()),
                reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
                replacement: None,
            },
            original,
            move |context| load_configuration(&context, input),
        )
        .map_err(|rejected| rejected.error)?;
    let completion = wake.completion(&client, &mut job)?;
    let JobOutcome::Failure(failure) = completion.outcome else {
        return Err("Oversized startup input reached schema parsing".into());
    };
    assert!(matches!(
        failure.cause(),
        EntryPreparationCause::Factory(ConfigurationCause::TooLarge)
    ));
    assert_eq!(
        std::fs::metadata(directory.join("oversized.toml"))?.len(),
        (MAX_STARTUP_CONFIGURATION_BYTES + 1) as u64
    );
    std::fs::remove_file(directory.join("oversized.toml"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}

#[test]
fn installed_descriptor_and_unicode_startup_input_share_original_preparation() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let directory = temporary_directory("input-pair")?;
    let mut paths = options()?.into_paths();
    paths[0] = directory.join("糖😀.toml");
    let input = StartupPaths::new(paths)?;
    std::fs::write(
        input.configuration(),
        br#"schema_version=1
mode="attach"
port=8188
profile="original_profile"
locale="en"
theme="dark"
reduced_motion=false
[attach]
origin="http://localhost:8188"
"#,
    )?;
    let descriptor = directory.join("launch.json");
    let expected = input.configuration().to_path_buf();
    std::fs::write(
        &descriptor,
        sugarsubstitute_configuration::descriptor::LaunchDescriptor::encode(input)?,
    )?;
    let original = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut job = client
        .submit_entry_preparation(
            &scope,
            JobSpec {
                id: JobSequence::new().allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(original.until()),
                reservation_bytes: CONFIGURATION_RESERVATION_BYTES,
                replacement: None,
            },
            original,
            move |context| {
                let paths =
                    installed::resolve(&context, ConfigurationSource::Installed(descriptor))?;
                load_configuration(&context, paths)
            },
        )
        .map_err(|rejected| rejected.error)?;
    let completion = wake.completion(&client, &mut job)?;
    let memory = completion.memory.clone();
    let JobOutcome::Success(prepared) = completion.outcome else {
        return Err("Installed input pair did not complete".into());
    };
    assert_eq!(prepared.value().options.configuration(), expected);
    assert!(memory.claim(1).is_err());
    drop(prepared);
    assert_eq!(
        memory.claim(CONFIGURATION_RESERVATION_BYTES)?.bytes(),
        CONFIGURATION_RESERVATION_BYTES
    );
    std::fs::remove_file(expected)?;
    std::fs::remove_file(directory.join("launch.json"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}
