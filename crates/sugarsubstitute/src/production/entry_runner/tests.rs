// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fail configuration before any environment capture and observe real runner joins.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(1);
pub(crate) struct NoChild;

#[test]
fn original_cleanup_wake_is_consumed_before_unsettled_owner_returns() -> Result<(), Box<dyn Error>>
{
    let installation = PathBuf::from(r"E:\unopened-installation");
    let command = ProductionCommand {
        launch_selection: None,
        diagnostics: Default::default(),
        installation: installation.clone(),
        options: super::super::options::ProductionEntryOptions {
            configuration: installation.join("unopened.toml"),
            package: installation.clone(),
            metadata: installation.clone(),
            cubes: installation.clone(),
            script: installation.clone(),
            profile: installation.clone(),
        }
        .into(),
    };
    let mut owner = EntryRunner::new(
        command,
        installation.join("unopened.exe"),
        Box::new(NoChild),
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    let issued = ShutdownBudget::new(sugarsubstitute_contracts::execution::Moment::from_nanos(0))?;
    let cutoff = Instant::now();
    owner.closing = true;
    owner.shutdown = Some(issued);
    owner.cleanup_deadline = Some(cutoff);
    assert!(!owner.consume_cleanup_cutoff(
        cutoff.checked_sub(Duration::from_nanos(1)).ok_or("Cutoff projection underflow")?
    ));
    assert!(owner.consume_cleanup_cutoff(cutoff));
    assert!(owner.next_deadline().is_none());
    assert!(!owner.consume_cleanup_cutoff(
        cutoff.checked_add(Duration::from_secs(100)).ok_or("Cutoff projection overflow")?
    ));
    assert_eq!(owner.shutdown, Some(issued));
    assert!(!owner.child_closed);
    assert!(!owner.intake_settled);
    assert!(!owner.finished);
    Ok(())
}

#[derive(Debug)]
struct ConstructorFailure;
impl fmt::Display for ConstructorFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original child construction failure")
    }
}
impl Error for ConstructorFailure {}

#[test]
fn startup_diagnostic_preserves_nested_causes_and_bounds_cyclic_sources()
-> Result<(), Box<dyn Error>> {
    struct CyclicCause;
    impl fmt::Debug for CyclicCause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("private retained payload")
        }
    }
    impl fmt::Display for CyclicCause {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("native operation failed")
        }
    }
    impl Error for CyclicCause {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(self)
        }
    }
    let original = sugarsubstitute_contracts::execution::ExecutionError::new(
        sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
        Box::new(ConstructorFailure),
    );
    let diagnostic = format!("{:?}", FailureDisplay(&original));
    assert!(diagnostic.contains("caused by: Original child construction failure"));
    let cyclic = format!("{:?}", FailureDisplay(&CyclicCause));
    assert_eq!(cyclic.matches("native operation failed").count(), 16);
    assert!(cyclic.ends_with("further causes retained"));
    assert!(!cyclic.contains("private retained payload"));
    let configuration = sugarsubstitute_comfy::configuration::parse_startup_configuration(
        b"unrecognized = \"retained-config-secret\"",
    )
    .err()
    .ok_or("Invalid configuration was admitted")?;
    assert!(configuration.source().is_some());
    let diagnostic = format!("{:?}", FailureDisplay(&configuration));
    assert!(!diagnostic.contains("retained-config-secret"));
    assert!(!diagnostic.contains("unrecognized"));
    for (cause, expected) in [
        (EntryTransportError::Closed, "Closed"),
        (EntryTransportError::InvalidDeadline, "InvalidDeadline"),
        (EntryTransportError::Authentication, "Authentication"),
    ] {
        assert_eq!(format!("{:?}", FailureDisplay(&cause)), expected);
    }
    Ok(())
}

#[test]
fn child_factory_failure_preserves_cause_before_native_pump_or_configuration()
-> Result<(), Box<dyn Error>> {
    let installation = PathBuf::from(r"E:\unopened-installation");
    let options = super::super::options::ProductionEntryOptions {
        configuration: installation.join("unopened.toml"),
        package: installation.clone(),
        metadata: installation.clone(),
        cubes: installation.clone(),
        script: installation.clone(),
        profile: installation.clone(),
    }
    .into();
    let command = ProductionCommand {
        launch_selection: None,
        installation: installation.clone(),
        diagnostics: Default::default(),
        options,
    };
    let calls = std::cell::Cell::new(0);
    let result = run(
        command,
        |command| {
            calls.set(calls.get() + 1);
            assert_eq!(command.installation, installation);
            Err::<Box<NoChild>, Box<dyn Error>>(Box::new(ConstructorFailure))
        },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    let cause = result.err().ok_or("Failed child constructor entered the native pump")?;
    assert_eq!(calls.get(), 1);
    assert!(cause.downcast_ref::<ConstructorFailure>().is_some());
    Ok(())
}
impl EntryActivation for NoChild {
    fn enter_activation(
        &mut self,
        _: &mut Option<AuthenticatedEntryLaunch>,
        _: EntryResources<'_>,
    ) -> Result<(), Box<dyn Error>> {
        Err("No synthetic test child admission exists".into())
    }
    fn consume(
        &mut self,
        _: &CompletionBatch,
        _: EntryResources<'_>,
    ) -> Result<HeadlessStep, Box<dyn Error>> {
        Ok(HeadlessStep::Active)
    }
    fn request_close(&mut self, _: Option<ShutdownBudget>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        None
    }
    fn normal_close_requested(&self) -> bool {
        false
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn poll_close(
        &mut self,
        _: &CompletionBatch,
        resources: Option<EntryResources<'_>>,
        _: ShutdownBudget,
    ) -> Result<bool, Box<dyn Error>> {
        Ok(resources.is_none())
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
#[test]
fn actual_runner_retains_schema_failure_and_joins_without_environment_capture()
-> Result<(), Box<dyn Error>> {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!("entry-runner-{}-{id}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let configuration = directory.join("configuration.json");
    std::fs::write(&configuration, b"{invalid-json")?;
    let options = super::super::options::ProductionEntryOptions {
        configuration,
        package: directory.clone(),
        metadata: directory.clone(),
        cubes: directory.clone(),
        script: directory.clone(),
        profile: directory.clone(),
    }
    .into();
    let command = ProductionCommand {
        launch_selection: None,
        installation: directory.clone(),
        diagnostics: Default::default(),
        options,
    };
    let child = super::super::entry_activation::ProductionActivation::new(
        directory.join("unopened.exe"),
        directory.clone(),
        directory.join("capture"),
    )?;
    let original_child = std::ptr::from_ref(child.as_ref());
    let result = run(command, |_| Ok(child), crate::diagnostics::DiagnosticHealth::fixture());
    let returned = match result {
        Err(cause) => cause,
        Ok(()) => return Err("Malformed actual configuration was admitted".into()),
    };
    let mut source = returned.source();
    let mut schema = false;
    while let Some(cause) = source {
        schema |= cause
            .downcast_ref::<sugarsubstitute_comfy::configuration::StartupConfigurationError>()
            .is_some();
        source = cause.source();
    }
    assert!(schema, "Actual schema failure was replaced during shutdown");
    let retained = returned
        .downcast_ref::<RunFailure<super::super::entry_activation::ProductionActivation>>()
        .ok_or("Original production child owner was replaced")?;
    assert_eq!(std::ptr::from_ref(retained.lifetime.child.as_ref()), original_child);
    assert!(retained.lifetime.finished, "Original runner did not observe cleanup and joins");
    assert!(!retained.lifetime.child.normal_close_requested());
    let diagnostic = format!("{returned:?}");
    assert!(diagnostic.contains("bootstrap_post_control"));
    assert!(diagnostic.contains("bootstrap_cleanup"));
    assert!(diagnostic.contains("authority_cleanup"));
    assert!(!diagnostic.contains("invalid-json"));
    drop(returned);
    std::fs::remove_file(directory.join("configuration.json"))?;
    std::fs::remove_dir(directory)?;
    Ok(())
}

#[path = "scope_admission_tests.rs"]
mod scope_admission;

#[path = "retained_child_tests.rs"]
mod retained_child;
