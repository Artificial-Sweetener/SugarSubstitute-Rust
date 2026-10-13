// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test exact input projection; borrowed wire shape creates no native authority.
use super::*;
use std::{
    num::NonZeroU16,
    os::windows::ffi::OsStrExt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, Presentation, Theme},
    coordinator_entry::{EntryArgumentsView, EntryEnvironmentView, EntryFrontendView},
};
type TestResult = Result<(), Box<dyn Error>>;

struct Release(Arc<AtomicU64>);
impl sugarsubstitute_contracts::execution::MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

fn bytes(units: impl IntoIterator<Item = u16>) -> Vec<u8> {
    units.into_iter().flat_map(u16::to_le_bytes).collect()
}
fn path_view(source: &[u8]) -> Result<EntryWindowsPathView<'_>, Box<dyn Error>> {
    Ok(EntryWindowsPathView::from_le_bytes(source)?)
}
fn view<'a>(
    connection: EntryConnectionView<'a>,
    directory: EntryWindowsPathView<'a>,
) -> Result<EntryLaunchRequestView<'a>, Box<dyn Error>> {
    Ok(EntryLaunchRequestView::new(
        connection,
        EntryFrontendView::new(directory, directory, directory, directory),
        directory,
        "Profile_1",
        Presentation::new("ja", Theme::Light, true)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?)
}

#[test]
fn attached_projection_preserves_explicit_origin_profile_and_presentation() -> TestResult {
    let directory = bytes("E:\\profile".encode_utf16());
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let projected = project(view(EntryConnectionView::Attached(origin), path_view(&directory)?)?)?;
    assert_eq!(projected.connection(), &ConnectionConfiguration::Attach(origin));
    assert_eq!(projected.profile().as_str(), "Profile_1");
    assert_eq!(projected.presentation(), Presentation::new("ja", Theme::Light, true)?);
    Ok(())
}

#[test]
fn managed_projection_preserves_raw_windows_units_and_exact_unicode_arguments() -> TestResult {
    let mut root_units: Vec<u16> = "E:\\Comfy".encode_utf16().collect();
    root_units.push(0xd800);
    let root = bytes(root_units.iter().copied());
    let interpreter = bytes("E:\\Comfy\\python.exe".encode_utf16());
    let argument = "--name=日本語 🧁";
    let mut arguments = Vec::new();
    arguments.extend_from_slice(&u32::try_from(argument.len())?.to_le_bytes());
    arguments.extend_from_slice(argument.as_bytes());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&arguments, 1)?,
        port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
    };
    let projected = project(view(requested, path_view(&interpreter)?)?)?;
    let ConnectionConfiguration::Launch(launch) = projected.connection() else {
        return Err("Managed input changed connection mode".into());
    };
    assert_eq!(launch.root().as_os_str().encode_wide().collect::<Vec<_>>(), root_units);
    assert_eq!(launch.interpreter(), std::path::Path::new("E:\\Comfy\\python.exe"));
    assert_eq!(launch.arguments(), &[argument]);
    assert_eq!(launch.port().get(), 8188);
    Ok(())
}

#[test]
fn borrowed_shape_does_not_bypass_authoritative_relative_path_rejection() -> TestResult {
    let root = bytes("relative".encode_utf16());
    let interpreter = bytes("E:\\python.exe".encode_utf16());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&[], 0)?,
        port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
    };
    assert!(matches!(
        project(view(requested, path_view(&interpreter)?)?),
        Err(ProjectionError::Configuration(_))
    ));
    Ok(())
}

#[test]
fn main_input_keeps_exact_windows_path_and_original_charge_until_drop() -> TestResult {
    let mut units: Vec<u16> = "E:\\profile".encode_utf16().collect();
    units.push(0xd800);
    let directory = bytes(units.iter().copied());
    let released = Arc::new(AtomicU64::new(0));
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(Release(released.clone())));
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let main = project_main(
        view(EntryConnectionView::Attached(origin), path_view(&directory)?)?,
        &memory,
    )?;
    assert_eq!(main.directory().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(main.profile().as_str(), "Profile_1");
    drop(memory);
    assert_eq!(released.load(Ordering::Acquire), 0);
    drop(main);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}

#[test]
fn exhausted_original_claim_rejects_main_projection_without_changing_input() -> TestResult {
    let directory = bytes("E:\\profile".encode_utf16());
    let released = Arc::new(AtomicU64::new(0));
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(Release(released.clone())));
    let occupied = memory.claim(RESERVATION_BYTES - MAIN_INPUT_BYTES + 1)?;
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let original = view(EntryConnectionView::Attached(origin), path_view(&directory)?)?;
    assert!(matches!(project_main(original, &memory), Err(ProjectionError::Memory(_))));
    assert_eq!(
        original.profile_directory().units().collect::<Vec<_>>(),
        "E:\\profile".encode_utf16().collect::<Vec<_>>()
    );
    drop(memory);
    assert_eq!(released.load(Ordering::Acquire), 0);
    drop(occupied);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}
