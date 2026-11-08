// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installation-owned managed projection stays identical to the sole Comfy launch owner.
use super::*;

#[test]
fn owned_managed_projection_matches_the_sole_launch_owner_and_names_the_selected_cubes()
-> TestResult {
    let root = bytes(r"E:\Comfy".encode_utf16());
    let interpreter = bytes(r"E:\Comfy\python.exe".encode_utf16());
    let cubes = bytes(r"E:\Comfy\custom_nodes\SugarCubes".encode_utf16());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&[], 0)?,
        port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
    };
    let source = EntryLaunchRequestView::new(
        requested,
        EntryFrontendView::new(
            path_view(&root)?,
            path_view(&root)?,
            path_view(&cubes)?,
            path_view(&root)?,
        ),
        path_view(&root)?,
        "Profile_1",
        Presentation::new("en", Theme::Dark, false)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?;
    let owned = project(source, BackendDataDisposition::InstallationOwned)?;
    let external = project(source, BackendDataDisposition::External)?;
    let (ConnectionConfiguration::Launch(owned), ConnectionConfiguration::Launch(external)) =
        (owned.connection(), external.connection())
    else {
        return Err("Managed input changed connection mode".into());
    };
    let request = LaunchConfiguration::new(
        r"E:\Comfy".into(),
        r"E:\Comfy\python.exe".into(),
        Vec::new(),
        8188,
    )?;
    let cubes_path = std::path::PathBuf::from(r"E:\Comfy\custom_nodes\SugarCubes");
    assert_eq!(
        owned,
        &sugarsubstitute_comfy::launch::effective_managed_launch(
            &request,
            BackendDataDisposition::InstallationOwned,
            &cubes_path,
        )?
    );
    assert_ne!(owned, external, "owned execution must not reuse the external bootstrap");
    assert!(
        owned.arguments().iter().any(|argument| argument == r"E:\Comfy\custom_nodes\SugarCubes")
    );
    Ok(())
}
