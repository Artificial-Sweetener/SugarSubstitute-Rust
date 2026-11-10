// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check inert native identity shape and original monotonic entry budgets.

use super::*;
use crate::{
    execution::{Moment, TimeError},
    lifecycle::NamespaceId,
};
use std::error::Error;

fn little_endian(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

#[test]
fn borrowed_paths_preserve_unpaired_surrogates_without_scalar_decoding()
-> Result<(), EntryShapeError> {
    let bytes = little_endian(&[69, 58, 92, 0xd800, 97, 0xdc01]);
    let view = EntryWindowsPathView::from_le_bytes(&bytes)?;
    assert_eq!(view.le_bytes(), bytes);
    assert_eq!(view.units().collect::<Vec<_>>(), [69, 58, 92, 0xd800, 97, 0xdc01]);
    assert_eq!(view.unit_len(), 6);
    assert_eq!(format!("{view:?}"), "EntryWindowsPathView { .. }");
    Ok(())
}

#[test]
fn raw_path_limit_accepts_exact_bound_and_rejects_nul_empty_odd_excess() {
    assert!(
        EntryWindowsPathView::from_le_bytes(&little_endian(&vec![1; MAX_ENTRY_PATH_UNITS])).is_ok()
    );
    for bytes in [&[][..], &[1][..], &[0, 0][..]] {
        assert!(matches!(EntryWindowsPathView::from_le_bytes(bytes), Err(EntryShapeError::Path)));
    }
    assert!(matches!(
        EntryWindowsPathView::from_le_bytes(&little_endian(&vec![1; MAX_ENTRY_PATH_UNITS + 1])),
        Err(EntryShapeError::Path)
    ));
}

#[test]
fn environment_order_hidden_drive_and_unpaired_values_remain_exact() -> Result<(), EntryShapeError>
{
    let snapshot = [90, 61, 0xd800, 0, 65, 61, 50, 0, 61, 67, 58, 61, 67, 58, 92, 0, 0];
    let bytes = little_endian(&snapshot);
    let view = EntryEnvironmentView::from_le_bytes(&bytes)?;
    assert_eq!(view.le_bytes(), bytes);
    assert_eq!(view.units().collect::<Vec<_>>(), snapshot);
    assert_eq!(view.unit_len(), snapshot.len());
    assert!(EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0]).is_ok());
    Ok(())
}

#[test]
fn malformed_environment_never_normalizes_or_truncates() {
    for snapshot in [
        vec![],
        vec![0],
        vec![65, 61, 66, 0],
        vec![65, 0, 0],
        vec![61, 0, 0],
        vec![0, 65, 61, 66, 0, 0],
        vec![0, 0, 0],
        vec![65, 61, 66, 0, 0, 67],
    ] {
        assert!(matches!(
            EntryEnvironmentView::from_le_bytes(&little_endian(&snapshot)),
            Err(EntryShapeError::Environment)
        ));
    }
    assert!(matches!(EntryEnvironmentView::from_le_bytes(&[0]), Err(EntryShapeError::Environment)));
    assert!(matches!(
        EntryEnvironmentView::from_le_bytes(&little_endian(&vec![
            1;
            MAX_ENTRY_ENVIRONMENT_UNITS + 1
        ])),
        Err(EntryShapeError::Environment)
    ));
}

fn argument_bytes(arguments: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    for argument in arguments {
        bytes.extend_from_slice(&u32::try_from(argument.len())?.to_le_bytes());
        bytes.extend_from_slice(argument.as_bytes());
    }
    Ok(bytes)
}

#[test]
fn borrowed_arguments_keep_unicode_empty_argument_and_exact_order() -> Result<(), Box<dyn Error>> {
    let arguments = ["", "--name", "文 😀", "$(literal)"];
    let bytes = argument_bytes(&arguments)?;
    let view = EntryArgumentsView::from_encoded(&bytes, 4)?;
    assert_eq!(view.encoded(), bytes);
    assert_eq!(view.iter().collect::<Vec<_>>(), arguments);
    assert_eq!(view.iter().len(), 4);
    assert_eq!(view.len(), 4);
    assert!(!view.is_empty());
    assert!(EntryArgumentsView::from_encoded(&[], 0)?.is_empty());
    Ok(())
}

#[test]
fn encoded_arguments_reject_malformed_section_and_shared_limits() -> Result<(), Box<dyn Error>> {
    use crate::configuration::ConfigurationError;
    for bytes in [vec![0], vec![4, 0, 0, 0, b'a'], vec![1, 0, 0, 0, 0xff], vec![0, 0, 0, 0, 1]] {
        assert!(matches!(
            EntryArgumentsView::from_encoded(&bytes, 1),
            Err(EntryShapeError::Arguments)
        ));
    }
    assert!(matches!(
        EntryArgumentsView::from_encoded(&argument_bytes(&["a\0b"])?, 1),
        Err(EntryShapeError::Configuration(ConfigurationError::ArgumentContainsNul))
    ));
    assert!(matches!(
        EntryArgumentsView::from_encoded(&argument_bytes(&[""; 65])?, 65),
        Err(EntryShapeError::Configuration(ConfigurationError::TooManyArguments))
    ));
    Ok(())
}

#[test]
fn borrowed_view_uses_profile_owner_and_keeps_exact_fields() -> Result<(), Box<dyn Error>> {
    use crate::configuration::{ConfigurationError, LoopbackOrigin, Presentation, Theme};
    let bytes = little_endian(&[69, 58, 92, 0xd800]);
    let path = EntryWindowsPathView::from_le_bytes(&bytes)?;
    let frontend = EntryFrontendView::new(path, path, path, path);
    let environment = EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?;
    let presentation = Presentation::new("en", Theme::Dark, true)?;
    let connection =
        EntryConnectionView::Attached(LoopbackOrigin::parse("http://localhost:8188", 8188)?);
    let view = EntryLaunchRequestView::new(
        connection,
        frontend,
        path,
        "Profile_1",
        presentation,
        environment,
    )?;
    assert_eq!(view.profile(), "Profile_1");
    assert_eq!(view.presentation(), presentation);
    assert_eq!(view.frontend().backend_script().le_bytes(), bytes);
    assert!(matches!(
        EntryLaunchRequestView::new(
            connection,
            frontend,
            path,
            "../bad",
            presentation,
            environment
        ),
        Err(EntryShapeError::Configuration(ConfigurationError::InvalidProfile))
    ));
    Ok(())
}

fn owned_request(snapshot: Vec<u16>) -> Result<EntryLaunchRequest, Box<dyn Error>> {
    use crate::configuration::{
        ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration,
        Theme,
    };
    #[cfg(windows)]
    let path = std::path::PathBuf::from(r"E:\entry-inert-input");
    #[cfg(not(windows))]
    let path = std::path::PathBuf::from("/entry-inert-input");
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("Profile_1".to_owned())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    Ok(EntryLaunchRequest::new(
        configuration,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path,
        snapshot,
    )?)
}

#[test]
fn originals_reference_keeps_fixed_matching_data_without_consuming_dynamic_request_capacity()
-> Result<(), Box<dyn Error>> {
    let request = owned_request(vec![0, 0])?;
    let retained = request.retained_bytes();
    assert!(request.originals().is_none());
    let reference = EntryOriginalsReference::new([7; 32])?;
    let request = request.with_originals(reference);
    assert_eq!(request.originals(), Some(reference));
    assert_eq!(request.retained_bytes(), retained);
    assert_eq!(MAX_ENTRY_REQUEST_ALLOCATION_BYTES, 100 * 1024);
    assert_eq!(request.into_parts().originals, Some(reference));
    assert_eq!(size_of::<EntryOriginalsReference>(), 32);
    assert_eq!(EntryOriginalsReference::new([0; 32]), Err(EntryShapeError::Identity));
    Ok(())
}

#[test]
fn owned_request_moves_original_snapshot_without_normalization() -> Result<(), Box<dyn Error>> {
    let snapshot = vec![90, 61, 0xd800, 0, 65, 61, 50, 0, 0];
    let request = owned_request(snapshot.clone())?;
    assert_eq!(request.environment(), snapshot);
    assert!(request.retained_bytes() <= MAX_ENTRY_REQUEST_ALLOCATION_BYTES);
    assert_eq!(request.into_parts().environment, snapshot);
    Ok(())
}

#[test]
fn spare_environment_capacity_is_rejected_before_retaining_request() {
    let mut snapshot = Vec::with_capacity(60 * 1024);
    snapshot.extend_from_slice(&[0, 0]);
    assert!(owned_request(snapshot).err().is_some_and(|error| {
        error.downcast_ref::<EntryShapeError>() == Some(&EntryShapeError::Capacity)
    }));
}

#[cfg(windows)]
#[test]
fn owned_managed_request_moves_isolated_high_and_low_surrogates_exactly()
-> Result<(), Box<dyn Error>> {
    use crate::configuration::{
        ConnectionConfiguration, LaunchConfiguration, Presentation, ProfileId,
        StartupConfiguration, Theme,
    };
    use std::{
        os::windows::ffi::{OsStrExt, OsStringExt},
        path::PathBuf,
    };
    let units = [69, 58, 92, 0xd800, 97, 0xdc01];
    let path = PathBuf::from(std::ffi::OsString::from_wide(&units));
    assert!(path.to_str().is_none());
    let launch = LaunchConfiguration::new(
        path.clone(),
        path.clone(),
        vec!["exact argument".to_owned()],
        8188,
    )?;
    let configuration = StartupConfiguration::new(
        ConnectionConfiguration::Launch(launch),
        ProfileId::new("Profile_1".to_owned())?,
        Presentation::new("en", Theme::Dark, false)?,
    );
    let request = EntryLaunchRequest::new(
        configuration,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path,
        vec![0, 0],
    )?;
    assert_eq!(request.package_directory().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    let parts = request.into_parts();
    let ConnectionConfiguration::Launch(launch) = parts.configuration.connection() else {
        return Err("managed request changed connection mode".into());
    };
    assert_eq!(launch.root().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(launch.interpreter().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(launch.arguments(), ["exact argument"]);
    assert_eq!(parts.profile_directory.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    Ok(())
}

#[test]
fn entry_ids_reject_zero_and_preserve_maximum_positive_fact() -> Result<(), EntryShapeError> {
    assert_eq!(EntryConnectionId::new(0), Err(EntryShapeError::Identity));
    assert_eq!(EntryOperationId::new(0), Err(EntryShapeError::Identity));
    assert_eq!(EntryRequestId::new(0), Err(EntryShapeError::Identity));
    assert_eq!(EntryConnectionId::new(u64::MAX)?.get(), u64::MAX);
    assert_eq!(EntryOperationId::new(u64::MAX)?.get(), u64::MAX);
    assert_eq!(EntryRequestId::new(u64::MAX)?.get(), u64::MAX);
    Ok(())
}

#[test]
fn exact_sid_facts_preserve_bytes_and_redact_account_image() -> Result<(), EntryShapeError> {
    let sid = [1, 1, 0, 0, 0, 0, 0, 5, 42, 0, 0, 0];
    let namespace = NamespaceId::new([9; 32]);
    let facts = EntryNativeIdentityFacts::new(namespace, &sid, 7, [3; 24])?;
    assert_eq!(facts.namespace(), namespace);
    assert_eq!(facts.account_sid(), sid);
    assert_eq!(facts.session_id(), 7);
    assert_eq!(facts.host_image_identity(), [3; 24]);
    assert_eq!(format!("{facts:?}"), "EntryNativeIdentityFacts { .. }");
    Ok(())
}

#[test]
fn sid_shape_rejects_truncated_extra_revision_and_subauthority_count() {
    for sid in [
        &[][..],
        &[1; 7][..],
        &[2, 0, 0, 0, 0, 0, 0, 5][..],
        &[1, 1, 0, 0, 0, 0, 0, 5][..],
        &[1, 0, 0, 0, 0, 0, 0, 5, 0][..],
        &[1, 16, 0, 0, 0, 0, 0, 5][..],
    ] {
        assert_eq!(
            EntryNativeIdentityFacts::new(NamespaceId::new([1; 32]), sid, 0, [0; 24]),
            Err(EntryShapeError::Identity)
        );
    }
}

#[test]
fn setup_budget_preserves_original_ancestor_across_shortened_probes() -> Result<(), Box<dyn Error>>
{
    let budget = EntrySetupBudget::new(Moment::from_nanos(100))?;
    assert_eq!(budget.started(), Moment::from_nanos(100));
    assert_eq!(budget.until(), Moment::from_nanos(3_600_000_000_100));
    assert_eq!(budget.probe_until(Moment::from_nanos(100))?, Moment::from_nanos(2_000_000_100));
    assert_eq!(budget.probe_until(Moment::from_nanos(3_599_000_000_100))?, budget.until());
    assert_eq!(budget.until(), Moment::from_nanos(3_600_000_000_100));
    Ok(())
}

#[test]
fn probe_rejects_clock_regression_and_expiry_equality() -> Result<(), TimeError> {
    let budget = EntrySetupBudget::new(Moment::from_nanos(100))?;
    assert_eq!(
        budget.probe_until(Moment::from_nanos(99)),
        Err(EntryBudgetError::Clock(TimeError::Regression))
    );
    assert_eq!(budget.probe_until(budget.until()), Err(EntryBudgetError::Expired));
    assert_eq!(
        budget.probe_until(Moment::from_nanos(3_600_000_000_101)),
        Err(EntryBudgetError::Expired)
    );
    assert!(EntryBudgetError::Clock(TimeError::Regression).source().is_some());
    Ok(())
}

#[test]
fn near_clock_limit_caps_remaining_before_addition() -> Result<(), Box<dyn Error>> {
    let budget = EntrySetupBudget::new(Moment::from_nanos(u64::MAX - 3_600_000_000_000))?;
    assert_eq!(budget.until(), Moment::from_nanos(u64::MAX));
    assert_eq!(budget.probe_until(Moment::from_nanos(u64::MAX - 1))?, budget.until());
    assert_eq!(
        EntrySetupBudget::new(Moment::from_nanos(u64::MAX - 3_599_999_999_999)),
        Err(TimeError::Overflow)
    );
    Ok(())
}
