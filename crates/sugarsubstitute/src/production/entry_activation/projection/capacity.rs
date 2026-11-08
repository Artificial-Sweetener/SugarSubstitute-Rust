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
/// Keep the selected user path within the existing field-construction allocation.
pub(super) fn field_available(input: &ProjectionInput) -> Result<u64, LifecycleRejection> {
    input
        ._charge
        .as_ref()
        .ok_or(LifecycleRejection::WrongPhase)?
        .bytes()
        .checked_sub(count(input.user.capacity())?)
        .ok_or(LifecycleRejection::Overloaded)
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
    // Desktop storage overrides can fill a native block even for an empty captured snapshot.
    bytes = add(bytes, 256 * 1024)?;
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
        bytes = add(bytes, mul(path_peak(root)?, 6)?)?;
        bytes = add(bytes, mul(path_peak(interpreter)?, 5)?)?;
        // Local storage preparation can fill a native environment even from an empty snapshot.
        bytes = add(bytes, 256 * 1024)?;
        let mut argument_bytes = 0;
        for argument in arguments.iter() {
            argument_bytes = add(argument_bytes, count(argument.len())?)?;
        }
        bytes = add(bytes, mul(argument_bytes, 3)?)?;
        bytes = add(
            bytes,
            vector(
                add(
                    count(arguments.len())?,
                    sugarsubstitute_comfy::launch::MANAGED_PREFIX_ARGUMENTS as u64,
                )?,
                size_of::<String>() as u64,
            )?,
        )?;
        bytes = add(bytes, vector(count(arguments.len())?, size_of::<String>() as u64)?)?;
        bytes = add(bytes, 128 + sugarsubstitute_comfy::launch::MANAGED_BOOTSTRAP_BYTES as u64)?;
    }
    let units = count(view.environment().unit_len())?;
    // Raw snapshot, desktop/backend output and managed PATH scratch/reallocation.
    // Entry is two slice-shaped fields in the source-pinned environment::entries owner.
    let environment_copies =
        if matches!(view.connection(), EntryConnectionView::Managed { .. }) { 12 } else { 6 };
    bytes = add(bytes, mul(units, environment_copies)?)?;
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
    identity: &sugarsubstitute_contracts::installation::InstallationId,
) -> Result<
    (super::super::funding::ProjectionDataFunding, super::super::funding::ProjectionPolicyCredit),
    Box<dyn Error + Send + Sync>,
> {
    let capture = count(input.capture_root.capacity())?;
    let retained = add(add(fields.allocated_bytes, capture)?, count(identity.allocated_bytes())?)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    #[test]
    fn selected_user_path_retains_its_original_field_allocation() -> Result<(), Box<dyn Error>> {
        let memory = MemoryLease::new(4096, Box::new(Release));
        let mut input = ProjectionInput::without_request();
        input._charge = Some(memory.claim(4096)?);
        input.user.try_reserve_exact(4097)?;
        assert!(matches!(field_available(&input), Err(LifecycleRejection::Overloaded)));
        input.user = PathBuf::new();
        assert_eq!(field_available(&input)?, 4096);
        input.user.try_reserve_exact(128)?;
        assert!(field_available(&input)? <= 4096 - 128);
        assert!(memory.claim(1).is_err());
        Ok(())
    }
}
