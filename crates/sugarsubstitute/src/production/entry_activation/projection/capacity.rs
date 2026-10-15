// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound pure field construction before allocating, then record actual retained bytes.
use super::*;
use sugarsubstitute_contracts::lifecycle::LifecycleRejection;

fn add(left: u64, right: u64) -> Result<u64, LifecycleRejection> {
    left.checked_add(right).ok_or(LifecycleRejection::Overloaded)
}
fn mul(left: u64, right: u64) -> Result<u64, LifecycleRejection> {
    left.checked_mul(right).ok_or(LifecycleRejection::Overloaded)
}
fn count(value: usize) -> Result<u64, LifecycleRejection> {
    u64::try_from(value).map_err(|_| LifecycleRejection::Overloaded)
}
/// RawVec's pinned doubling/min-eight bound, including old/new allocation overlap.
fn vector(elements: u64, width: u64) -> Result<u64, LifecycleRejection> {
    if elements == 0 {
        return Ok(0);
    }
    mul(mul(add(mul(elements, 2)?, 8)?, 2)?, width)
}
fn path_peak(path: EntryWindowsPathView<'_>) -> Result<u64, LifecycleRejection> {
    let units = count(path.unit_len())?;
    // UTF16 scratch plus worst three-byte WTF8 output per unit and realloc overlap.
    add(mul(units, 2)?, vector(mul(units, 3)?, 1)?)
}
/// Bound actual input lengths before any field-constructor allocation. Requested
/// Global layouts are conservative; native effects and arbitrary causes are not
/// inferred from these byte facts. Large valid input may reject as Overloaded.
pub(super) fn preflight(
    view: EntryLaunchRequestView<'_>,
    capture: &PathBuf,
    available: u64,
) -> Result<(), LifecycleRejection> {
    let mut bytes = count(capture.capacity())?;
    bytes = add(bytes, path_peak(view.profile_directory())?)?;
    bytes = add(bytes, mul(count(view.profile_directory().unit_len())?, 2)?)?;
    bytes = add(bytes, count(view.profile().len())?)?;
    let frontend = view.frontend();
    for path in [
        frontend.package_directory(),
        frontend.metadata_file(),
        frontend.cube_directory(),
        frontend.backend_script(),
    ] {
        bytes = add(bytes, path_peak(path)?)?;
    }
    if let EntryConnectionView::Managed { root, interpreter, arguments, .. } = view.connection() {
        // Requested paths/argv coexist with their effective Comfy copies and
        // script-path preparation. The one actual owner remains comfy::launch.
        bytes = add(bytes, mul(path_peak(root)?, 4)?)?;
        bytes = add(bytes, mul(path_peak(interpreter)?, 3)?)?;
        let mut argument_bytes = 0;
        for argument in arguments.iter() {
            argument_bytes = add(argument_bytes, count(argument.len())?)?;
        }
        bytes = add(bytes, mul(argument_bytes, 3)?)?;
        bytes = add(bytes, vector(add(count(arguments.len())?, 7)?, size_of::<String>() as u64)?)?;
        bytes = add(bytes, vector(count(arguments.len())?, size_of::<String>() as u64)?)?;
        bytes = add(bytes, 128)?; // Seven fixed Comfy options/port, excluding script.
    }
    let units = count(view.environment().unit_len())?;
    // Raw snapshot, two output blocks, merge entries and typed controls. Entry is
    // two slice-shaped fields in the source-pinned environment::entries owner.
    bytes = add(bytes, mul(units, 6)?)?;
    let entry = (size_of::<&[u16]>() + size_of::<Option<&[u16]>>() + align_of::<&[u16]>()) as u64;
    bytes = add(bytes, mul(1024.min(units), entry)?)?;
    bytes = add(bytes, (size_of::<Fields>() + size_of::<ProjectedEntry>()) as u64)?;
    if bytes <= available { Ok(()) } else { Err(LifecycleRejection::Overloaded) }
}
/// Measure owned capacities before frontend paths enter their opaque source owner.
pub(super) fn frontend(paths: &[PathBuf; 4]) -> Result<u64, LifecycleRejection> {
    paths.iter().try_fold(0, |sum, path| add(sum, count(path.capacity())?))
}
pub(super) fn data(fields: &Fields, frontend: u64) -> Result<u64, Box<dyn Error + Send + Sync>> {
    let mut bytes = add(
        count(fields.requested_profile.capacity())?,
        mul(count(fields.profile_units.capacity())?, 2)?,
    )?;
    bytes = add(bytes, count(fields.profile.allocated_bytes())?)?;
    bytes = add(bytes, frontend)?;
    if let BackendTarget::Managed(target) = &fields.target {
        bytes =
            add(bytes, count(target.allocated_bytes().ok_or(LifecycleRejection::Overloaded)?)?)?;
    }
    bytes = add(bytes, fields.desktop_environment.allocated_bytes()?)?;
    if let Some(environment) = &fields.backend_environment {
        bytes = add(bytes, environment.allocated_bytes()?)?;
    }
    Ok(bytes)
}
pub(super) fn funding(
    input: &mut ProjectionInput,
    fields: &Fields,
) -> Result<
    (super::super::funding::ProjectionDataFunding, super::super::funding::ProjectionPolicyCredit),
    Box<dyn Error + Send + Sync>,
> {
    let capture = count(input.capture_root.capacity())?;
    let retained = add(fields.allocated_bytes, capture)?;
    // Copies preceding admission to capture/profile native owners. App target and
    // enrollment retention are included in actual policy preflight; its one
    // temporary ProfileId input copy is funded here before that input exists.
    // Custody invocation copies require the backend owner's separate checked
    // fixed-witness preflight; this quota does not certify that other role.
    let profile =
        sugarsubstitute_supervision::profiles::WindowsProfileLease::constructor_storage_bytes(
            &fields.requested_profile,
        )?;
    let future = add(
        add(add(capture, count(fields.requested_profile.capacity())?)?, profile)?,
        count(fields.profile.allocated_bytes())?,
    )?;
    super::super::funding::complete(
        &mut input._charge,
        super::super::funding::ProjectionDataBytes { retained, future },
    )
    .map_err(|cause| Box::new(cause) as _)
}
