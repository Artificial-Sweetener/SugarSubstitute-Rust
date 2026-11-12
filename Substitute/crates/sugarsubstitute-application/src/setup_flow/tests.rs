// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic native navigation and request-correlation qualification.
use super::*;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn flow() -> TestResult<SetupFlow> {
    Ok(SetupFlow::new("native".into())?)
}
fn primary(flow: &mut SetupFlow) -> TestResult<Request> {
    flow.primary(Duration::ZERO)?.ok_or_else(|| "Expected operation".into())
}
fn complete(
    flow: &mut SetupFlow,
    request: &Request,
    result: Completed,
) -> TestResult<Option<Request>> {
    Ok(flow.receive(request.id, Outcome::Completed(result), Duration::ZERO)?)
}
fn location(flow: &mut SetupFlow, existing: Option<ExistingInstallation>) -> TestResult {
    let request = primary(flow)?;
    complete(flow, &request, Completed::Locale)?;
    assert_eq!(flow.page(), Page::Location);
    let request = primary(flow)?;
    complete(
        flow,
        &request,
        Completed::Location(Location {
            comfy_root: "comfy".into(),
            models: "models".into(),
            outputs: "outputs".into(),
            existing,
        }),
    )?;
    Ok(())
}
fn choice(value: &str, recommended: bool) -> Choice {
    Choice {
        value: value.into(),
        label: format!("Label {value}"),
        description: "Description".into(),
        recommended,
        selection_token: Some(format!("opaque-{value}")),
    }
}
fn managed(flow: &mut SetupFlow) -> TestResult {
    location(flow, None)?;
    let release = primary(flow)?;
    let version = complete(flow, &release, Completed::Options(vec![choice("release", true)]))?
        .ok_or("Expected dependent operation")?;
    let variant = complete(flow, &version, Completed::Options(vec![choice("version", true)]))?
        .ok_or("Expected dependent operation")?;
    complete(flow, &variant, Completed::Options(vec![choice("variant", true)]))?;
    Ok(())
}
fn install(flow: &mut SetupFlow) -> TestResult<Request> {
    let runtime = primary(flow)?;
    assert!(matches!(runtime.operation, Operation::Runtime(_)));
    complete(flow, &runtime, Completed::Validated)?;
    assert_eq!(flow.page(), Page::Folders);
    let validate = primary(flow)?;
    assert!(matches!(validate.operation, Operation::Validate(_)));
    Ok(complete(flow, &validate, Completed::Validated)?.ok_or("Expected dependent operation")?)
}

#[test]
fn fresh_managed_flow_preserves_choices_and_correlates_open() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    assert_eq!(flow.journey(), (4, 7));
    assert_eq!(flow.choices().selection_token.as_deref(), Some("opaque-variant"));
    let request = install(&mut flow)?;
    assert!(matches!(request.operation, Operation::Install(_)));
    assert_eq!(flow.journey(), (6, 7));
    assert!(!flow.can_primary());
    complete(&mut flow, &request, Completed::Installed)?;
    assert_eq!(flow.journey(), (7, 7));
    assert_eq!(flow.overall(), 100.0);
    let open = primary(&mut flow)?;
    assert_eq!(open.operation, Operation::Open);
    assert_eq!(flow.primary(Duration::ZERO), Err(FlowError::ActionUnavailable));
    complete(&mut flow, &open, Completed::Opened)?;
    assert!(flow.close_requested());
    assert!(!flow.can_primary());
    Ok(())
}

#[test]
fn attached_flow_skips_options_and_keeps_exact_editable_paths() -> TestResult {
    let mut flow = flow()?;
    location(&mut flow, None)?;
    flow.set_mode(RuntimeMode::Attached)?;
    assert_eq!(flow.primary(Duration::ZERO)?, None);
    flow.set_path(PathField::Comfy, " E:\\我的环境 ".into())?;
    flow.set_path(PathField::Python, "python.exe".into())?;
    let request = install(&mut flow)?;
    let Operation::Install(choices) = request.operation else {
        return Err("Expected installation".into());
    };
    assert_eq!(choices.comfy_root, " E:\\我的环境 ");
    assert_eq!(choices.python, "python.exe");
    assert_eq!(choices.models, "models");
    assert_eq!(choices.selection_token, None);
    Ok(())
}

#[test]
fn maintenance_bypasses_provisioning_and_retries_same_inspected_root() -> TestResult {
    let mut flow = flow()?;
    location(
        &mut flow,
        Some(ExistingInstallation {
            install_root: "inspected-root".into(),
            comfy_root: "attached-comfy".into(),
            executable_missing: true,
            entry_missing: true,
        }),
    )?;
    assert_eq!(flow.journey(), (3, 5));
    let request = primary(&mut flow)?;
    assert_eq!(request.operation, Operation::Maintain("inspected-root".into()));
    flow.receive(request.id, Outcome::Failed("repair failed".into()), Duration::ZERO)?;
    assert_eq!(
        flow.primary_key(),
        Some(sugarsubstitute_contracts::localization::installer::InstallerText::Retry)
    );
    assert_eq!(flow.primary(Duration::ZERO)?, None);
    assert_eq!(flow.page(), Page::Maintenance);
    let retry = primary(&mut flow)?;
    assert_ne!(request.id, retry.id);
    assert_eq!(retry.operation, request.operation);
    Ok(())
}

#[test]
fn installation_retry_reloads_managed_options_without_losing_folders() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let runtime = primary(&mut flow)?;
    complete(&mut flow, &runtime, Completed::Validated)?;
    flow.set_path(PathField::Outputs, "my outputs".into())?;
    let validate = primary(&mut flow)?;
    let install = complete(&mut flow, &validate, Completed::Validated)?
        .ok_or("Expected dependent operation")?;
    flow.receive(install.id, Outcome::Cancelled, Duration::ZERO)?;
    let retry = primary(&mut flow)?;
    assert!(matches!(retry.operation, Operation::Options { field: OptionField::Release, .. }));
    assert_eq!(flow.page(), Page::Configuration);
    assert_eq!(flow.choices().outputs, "my outputs");
    assert!(!flow.can_primary());
    Ok(())
}

#[test]
fn stale_or_wrong_reply_does_not_consume_pending_work() -> TestResult {
    let mut flow = flow()?;
    let request = primary(&mut flow)?;
    assert_eq!(
        flow.receive(
            RequestId(request.id.get() + 1),
            Outcome::Completed(Completed::Locale),
            Duration::ZERO
        ),
        Err(FlowError::UnexpectedReply)
    );
    assert_eq!(
        flow.receive(request.id, Outcome::Completed(Completed::Installed), Duration::ZERO),
        Err(FlowError::UnexpectedReply)
    );
    assert!(flow.busy());
    assert_eq!(flow.page(), Page::Language);
    complete(&mut flow, &request, Completed::Locale)?;
    assert_eq!(flow.page(), Page::Location);
    assert_eq!(
        flow.receive(request.id, Outcome::Completed(Completed::Locale), Duration::ZERO),
        Err(FlowError::UnexpectedReply)
    );
    Ok(())
}

#[test]
fn open_failure_keeps_completion_and_allows_explicit_retry() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let request = install(&mut flow)?;
    complete(&mut flow, &request, Completed::Installed)?;
    let open = primary(&mut flow)?;
    flow.receive(open.id, Outcome::Failed("launch failed".into()), Duration::ZERO)?;
    assert_eq!(flow.page(), Page::Complete);
    assert_eq!(
        flow.primary_key(),
        Some(sugarsubstitute_contracts::localization::installer::InstallerText::Open)
    );
    assert!(!flow.close_requested());
    assert!(flow.can_primary());
    assert_eq!(primary(&mut flow)?.operation, Operation::Open);
    Ok(())
}

#[test]
fn progress_is_monotonic_and_slow_feedback_never_expires_work() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let request = install(&mut flow)?;
    for value in [45.0, 20.0, f64::NAN, -100.0, 101.0] {
        flow.progress(request.id, "Provisioning".into(), Some(value))?;
    }
    assert_eq!(flow.overall(), 99.0);
    assert!(!flow.waiting(Duration::from_secs(119)));
    assert!(flow.waiting(Duration::from_secs(120)));
    assert!(flow.waiting(Duration::from_secs(86_400)));
    assert!(flow.busy());
    assert_eq!(flow.failure(), None);
    complete(&mut flow, &request, Completed::Installed)?;
    assert!(!flow.waiting(Duration::from_secs(86_400)));
    assert_eq!(
        flow.progress(request.id, "late".into(), Some(1.0)),
        Err(FlowError::UnexpectedReply)
    );
    Ok(())
}

#[test]
fn option_chain_keeps_one_wait_clock_and_selects_last_recommendation() -> TestResult {
    let mut flow = flow()?;
    location(&mut flow, None)?;
    let release = flow.primary(Duration::from_secs(10))?.ok_or("Expected operation")?;
    let values = vec![choice("first", true), choice("second", true)];
    let version = flow
        .receive(
            release.id,
            Outcome::Completed(Completed::Options(values)),
            Duration::from_secs(100),
        )?
        .ok_or("Expected dependent operation")?;
    assert_eq!(flow.selected(OptionField::Release), Some(1));
    assert!(
        matches!(version.operation, Operation::Options { release: Some(ref value), .. } if value == "second")
    );
    assert!(flow.waiting(Duration::from_secs(130)));
    let variant = complete(&mut flow, &version, Completed::Options(vec![]))?
        .ok_or("Expected dependent operation")?;
    complete(&mut flow, &variant, Completed::Options(vec![]))?;
    assert_eq!(
        flow.failure(),
        Some(&Failure::Localized(
            sugarsubstitute_contracts::localization::installer::InstallerText::NoEnvironment
        ))
    );
    assert!(!flow.can_primary());
    assert!(!flow.busy());
    assert!(flow.can_back());
    Ok(())
}

#[test]
fn option_reselection_invalidates_token_until_new_variant_arrives() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let request =
        flow.select_option(OptionField::Release, 0, Duration::ZERO)?.ok_or("Expected operation")?;
    assert_eq!(flow.choices().selection_token, None);
    assert_eq!(flow.set_mirrors(true, Duration::ZERO), Err(FlowError::ActionUnavailable));
    let next =
        complete(&mut flow, &request, Completed::Options(vec![choice("new-version", false)]))?
            .ok_or("Expected dependent operation")?;
    complete(&mut flow, &next, Completed::Options(vec![choice("a", false), choice("b", false)]))?;
    assert_eq!(flow.selected(OptionField::Variant), Some(0));
    assert_eq!(flow.select_option(OptionField::Variant, 1, Duration::ZERO)?, None);
    assert_eq!(flow.choices().selection_token.as_deref(), Some("opaque-b"));
    assert_eq!(
        flow.select_option(OptionField::Variant, 2, Duration::ZERO),
        Err(FlowError::InvalidOptions)
    );
    Ok(())
}

#[test]
fn locale_is_committed_only_on_acceptance_and_pending_input_is_disabled() -> TestResult {
    let mut flow = flow()?;
    assert_eq!(flow.set_locale("unknown", Duration::ZERO), Err(FlowError::UnsupportedLocale));
    let request = flow.set_locale("es", Duration::ZERO)?;
    assert_eq!(flow.locale(), "en");
    assert_eq!(flow.set_locale("en", Duration::ZERO), Err(FlowError::ActionUnavailable));
    flow.receive(request.id, Outcome::Failed("rejected".into()), Duration::ZERO)?;
    assert_eq!(flow.locale(), "en");
    let request = flow.set_locale("es", Duration::ZERO)?;
    complete(&mut flow, &request, Completed::Locale)?;
    assert_eq!(flow.locale(), "es");
    assert_eq!(flow.page(), Page::Language);
    Ok(())
}

#[test]
fn invalid_text_and_option_inventory_are_rejected_without_mutation() -> TestResult {
    assert!(matches!(SetupFlow::new("bad\0path".into()), Err(FlowError::InvalidText)));
    let mut flow = flow()?;
    location(&mut flow, None)?;
    let request = primary(&mut flow)?;
    assert_eq!(
        flow.receive(
            request.id,
            Outcome::Completed(Completed::Options(vec![choice("a", false); 257])),
            Duration::ZERO
        ),
        Err(FlowError::InvalidOptions)
    );
    assert!(flow.busy());
    assert!(flow.options(OptionField::Release).is_empty());
    let bad = Choice { label: "\0".into(), ..choice("a", false) };
    assert_eq!(
        flow.receive(request.id, Outcome::Completed(Completed::Options(vec![bad])), Duration::ZERO),
        Err(FlowError::InvalidText)
    );
    assert!(flow.busy());
    Ok(())
}

#[test]
fn worker_loss_and_close_seal_actions_but_leave_details_available() -> TestResult {
    let mut flow = flow()?;
    let request = primary(&mut flow)?;
    flow.unavailable();
    assert_eq!(flow.page(), Page::Progress);
    assert!(flow.details_open());
    assert!(!flow.can_primary());
    assert!(!flow.can_back());
    assert_eq!(
        flow.receive(request.id, Outcome::Completed(Completed::Locale), Duration::ZERO),
        Err(FlowError::UnexpectedReply)
    );
    flow.closing();
    flow.unavailable();
    assert_eq!(flow.status(), &Status::Cancelling);
    assert_eq!(flow.primary_key(), None);
    flow.set_details(false);
    assert!(!flow.details_open());
    Ok(())
}

#[test]
fn back_resets_viewport_revision_and_retains_fields() -> TestResult {
    let mut flow = flow()?;
    location(&mut flow, None)?;
    let revision = flow.page_revision();
    flow.back()?;
    assert_eq!(flow.page(), Page::Location);
    assert!(flow.page_revision() > revision);
    flow.set_path(PathField::Installation, "changed".into())?;
    flow.back()?;
    assert_eq!(flow.choices().install_root, "changed");
    assert_eq!(flow.back(), Err(FlowError::ActionUnavailable));
    Ok(())
}

#[test]
fn validation_failure_retains_folders_without_admitting_installation() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let runtime = primary(&mut flow)?;
    complete(&mut flow, &runtime, Completed::Validated)?;
    flow.set_path(PathField::Models, "user models".into())?;
    let validation = primary(&mut flow)?;
    assert_eq!(
        flow.set_path(PathField::Models, "changed".into()),
        Err(FlowError::ActionUnavailable)
    );
    assert_eq!(flow.back(), Err(FlowError::ActionUnavailable));
    flow.receive(validation.id, Outcome::Failed("invalid folder".into()), Duration::ZERO)?;
    assert_eq!(flow.page(), Page::Folders);
    assert_eq!(
        flow.primary_key(),
        Some(sugarsubstitute_contracts::localization::installer::InstallerText::Install)
    );
    assert_eq!(flow.choices().models, "user models");
    assert!(matches!(primary(&mut flow)?.operation, Operation::Validate(_)));
    Ok(())
}

#[test]
fn attached_retry_returns_to_paths_without_provisioning_option_requests() -> TestResult {
    let mut flow = flow()?;
    location(&mut flow, None)?;
    flow.set_mode(RuntimeMode::Attached)?;
    assert_eq!(flow.primary(Duration::ZERO)?, None);
    flow.set_path(PathField::Python, "existing-python".into())?;
    let installation = install(&mut flow)?;
    flow.receive(installation.id, Outcome::Failed("failed".into()), Duration::ZERO)?;
    assert_eq!(flow.primary(Duration::ZERO)?, None);
    assert_eq!(flow.page(), Page::Configuration);
    assert_eq!(flow.choices().python, "existing-python");
    assert!(flow.can_primary());
    Ok(())
}

#[test]
fn mirror_reload_preserves_preference_and_invalidates_previous_selection() -> TestResult {
    let mut flow = flow()?;
    managed(&mut flow)?;
    let request = flow.set_mirrors(true, Duration::from_secs(50))?;
    assert!(matches!(
        request.operation,
        Operation::Options { field: OptionField::Release, mirrors: true, .. }
    ));
    assert!(flow.mirrors());
    assert_eq!(flow.choices().selection_token, None);
    assert!(!flow.can_primary());
    assert!(!flow.waiting(Duration::from_secs(169)));
    assert!(flow.waiting(Duration::from_secs(170)));
    flow.receive(request.id, Outcome::Failed("discovery failed".into()), Duration::ZERO)?;
    assert!(!flow.can_primary());
    assert!(flow.can_back());
    Ok(())
}

#[test]
fn options_have_aggregate_memory_bound_in_addition_to_per_field_limits() -> TestResult {
    let mut flow = flow()?;
    location(&mut flow, None)?;
    let request = primary(&mut flow)?;
    let large = Choice { description: "x".repeat(4096), ..choice("a", false) };
    assert_eq!(
        flow.receive(
            request.id,
            Outcome::Completed(Completed::Options(vec![large; 17])),
            Duration::ZERO
        ),
        Err(FlowError::InvalidOptions)
    );
    assert!(flow.busy());
    assert!(flow.options(OptionField::Release).is_empty());
    Ok(())
}

/// Shell-input failures keep the user's choices and route; busy/closing flows reject new feedback.
#[test]
fn native_input_failure_is_recoverable_without_changing_choices() -> TestResult {
    let mut flow = flow()?;
    let choices = flow.choices().clone();
    flow.input_failed("Picker failed".into())?;
    assert_eq!(flow.choices(), &choices);
    assert_eq!(flow.page(), Page::Language);
    assert!(
        matches!(flow.failure(), Some(Failure::Message(message)) if message == "Picker failed")
    );
    assert!(flow.input_failed("x".repeat(4097)).is_err());
    let _request = primary(&mut flow)?;
    assert!(flow.failure().is_none());
    assert!(flow.input_failed("Late failure".into()).is_err());
    flow.closing();
    assert!(flow.input_failed("Closed".into()).is_err());
    Ok(())
}
