// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject ad-hoc toolkit, native, macro and dependency-alias presentation paths.

use super::*;
use crate::{
    error::Failure,
    fixtures::{Repository, write},
    policy::Owner,
};

/// Own isolated UI and consuming crates without invoking a compiler or changing real policy.
fn fixture() -> Result<(Repository, Policy)> {
    let mut fixture = Repository::new()?;
    for root in ["tools", "native", "installer"] {
        write(fixture.root(), &format!("{root}/.fixture"), "")?;
    }
    fixture.architecture.owners[0].layer = "adapter".into();
    fixture.architecture.owners[0].concern =
        "Shared native widget rendering and interaction rules".into();
    fixture.architecture.owners.push(Owner {
        package: "consumer".into(),
        root: "crates/consumer".into(),
        auxiliary_roots: vec![],
        auxiliary_files: vec![],
        layer: "adapter".into(),
        concern: "Native platform boundary".into(),
    });
    write(fixture.root(), "crates/consumer/src/lib.rs", "pub fn harmless() {}")?;
    fixture.workspace = serde_json::from_value(serde_json::json!({
        "workspace_members": ["host", "consumer"],
        "packages": [
            {"name":"host", "id":"host", "manifest_path":fixture.root().join("crates/host/Cargo.toml"), "dependencies":[]},
            {"name":"consumer", "id":"consumer", "manifest_path":fixture.root().join("crates/consumer/Cargo.toml"), "dependencies":[]}
        ]
    })).map_err(|e| Failure::external("construct UI fixture", e))?;
    Ok((
        fixture,
        Policy {
            schema_version: 1,
            owner: "host".into(),
            dependencies: vec![],
            boundaries: vec![],
        },
    ))
}

/// Native/tooling/worker Rust and loose mixed-case files retain their actual owner and review.
#[test]
fn auxiliary_roots_and_loose_files_cannot_hide_native_construction() -> Result<()> {
    for path in ["native/widget.RS", "tools/compose/window.rs", "installer/widgets/peer.rs"] {
        let (mut fixture, mut policy) = fixture()?;
        fixture.architecture.owners[1].auxiliary_files.push(path.into());
        write(fixture.root(), path, "fn run(){ CreateWindowExW(); }")?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
        let sources = vec![path.into()];
        policy.boundaries.push(Boundary {
            owner: "consumer".into(),
            concern: "Reviewed native peer".into(),
            source_sha256: fingerprint(fixture.root(), &sources)?,
            sources,
            capabilities: vec![Capability::Window],
        });
        check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
        fixture.architecture.owners[1].auxiliary_files.clear();
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    Ok(())
}

/// Canonical imports, aliases, wildcard use, macro bodies and target/test branches are all code.
#[test]
fn source_capabilities_cannot_hide_in_aliases_macros_or_platform_branches() -> Result<()> {
    for source in [
        "use windows::Win32::UI::WindowsAndMessaging::CreateWindowExW as create;",
        "use windows::Win32::UI::WindowsAndMessaging::CreateCursor as make; fn run(){ make(); }",
        "macro_rules! cursor { () => { CreateCursor() }; }",
        "unsafe extern \"system\" { #[link_name=\"CreateCursor\"] fn make(); }",
        "use windows::Win32::UI::WindowsAndMessaging::*; fn run(){ CreateWindowExW(); }",
        "macro_rules! ui { () => { WindowBuilder::new() }; }",
        "macro_rules! ui { ($f:ident) => { $f() }; } fn run(){ ui!(r#CreateWindowExW); }",
        "#[cfg(target_os=\"linux\")] fn run(){ tiny_skia::Pixmap::new(10,10); }",
        "#[cfg(test)] fn run(){ WebViewBuilder::new(); }",
        "unsafe extern \"system\" { #[link_name=\"CreateWindowExW\"] fn build(); }",
        "macro_rules! native { () => { unsafe extern \"system\" { #[link_name=\"CreateWindowExW\"] fn build(); } }; }",
        "use sugarsubstitute_ui::shapes::rounded as make;",
        "macro_rules! shape { () => { use shapes::{rounded}; rounded(); }; }",
        "fn draw(){ target.rounded(); }",
        "use sugarsubstitute_ui::shapes::blend_pixel as paint;",
        "use sugarsubstitute_ui::bitmap::Bitmap as Pixels; fn run(){Pixels::new(1,1);}",
        "use sugarsubstitute_ui::bitmap::CoverageMask as Clip;",
        "fn run(target: NativeTarget) { target.FillRoundedRectangle(); }",
        "fn run(target: NativeTarget) { target.FillRectangle(); }",
        "use windows::Win32::Graphics::Gdi::SetTextColor as recolor;",
        "macro_rules! palette { () => { SetBkColor(); SetDCBrushColor(); }; }",
        "#[cfg(target_os=\"macos\")] fn run(){ wgpu::Instance::new(options); }",
        "macro_rules! gpu { () => { use wgpu as rendering; rendering::Device }; }",
    ] {
        assert!(!ui_source::findings(source, &[])?.is_empty(), "accepted {source}");
    }
    assert!(
        ui_source::findings("// WindowBuilder\nfn run(){ let _ = \"CreateWindowExW\"; }", &[])?
            .is_empty()
    );
    assert!(ui_source::findings("fn pan(){ let rounded = (1.0_f64 + 0.5).floor(); if rounded < 2.0 { let phase = rounded as i32; } }", &[])?.is_empty());
    Ok(())
}

/// Toolkit aliases are classified by Cargo identity, not by their local name.
#[test]
fn renamed_toolkit_dependency_and_its_source_are_rejected() -> Result<()> {
    for name in ["tiny-skia", "wgpu"] {
        let (mut fixture, policy) = fixture()?;
        fixture.workspace.packages[1].dependencies.push(crate::workspace::Dependency {
            name: name.into(),
            rename: Some("drawing".into()),
            kind: None,
            path: None,
        });
        let dependencies = &fixture.workspace.packages[1].dependencies;
        let source = if name == "wgpu" {
            "fn run(){ drawing::Instance::new(options); }"
        } else {
            "fn run(){ drawing::Pixmap::new(10,10); }"
        };
        assert_eq!(
            ui_source::findings(source, dependencies)?,
            BTreeSet::from([Capability::Raster])
        );
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    Ok(())
}

/// New source cannot inherit arbitrary UI authority from an existing dependency or crate.
#[test]
fn unregistered_native_construction_is_rejected() -> Result<()> {
    for source in ["fn run(){ CreateWindowExW(); }", "fn run(){ CreateCursor(); }"] {
        let (fixture, policy) = fixture()?;
        check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
        write(fixture.root(), "crates/consumer/src/adhoc.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    Ok(())
}

/// Exact reviewed sources admit the platform boundary; edits require renewed review.
#[test]
fn registered_boundary_rejects_new_source_and_changed_contents() -> Result<()> {
    let (fixture, mut policy) = fixture()?;
    let path = "crates/consumer/src/lib.rs";
    write(fixture.root(), path, "fn run(){ CreateWindowExW(); }")?;
    let sources = vec![path.into()];
    policy.boundaries.push(Boundary {
        owner: "consumer".into(),
        concern: "Native peer creation".into(),
        source_sha256: fingerprint(fixture.root(), &sources)?,
        sources,
        capabilities: vec![Capability::Window],
    });
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), "crates/consumer/src/adhoc.rs", "fn run(){ CreateWindowExW(); }")?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    write(fixture.root(), "crates/consumer/src/adhoc.rs", "fn harmless(){}")?;
    write(fixture.root(), path, "fn run(){ CreateWindowExW(); MessageBoxW(); }")?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    Ok(())
}

/// Widget rendering cannot become an alternate window or browser host.
#[test]
fn widget_owner_cannot_create_windows_or_webviews() -> Result<()> {
    let (fixture, policy) = fixture()?;
    for source in ["fn run(){ WindowBuilder::new(); }", "fn run(){ WebViewBuilder::new(); }"] {
        write(fixture.root(), "crates/host/src/lib.rs", source)?;
        assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    }
    write(fixture.root(), "crates/host/src/lib.rs", "fn run(){ target.fill_rect(); }")?;
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    Ok(())
}

/// An approved source cannot retain capabilities it does not consume or name another owner.
#[test]
fn boundary_authority_is_exact_and_cannot_cross_owners() -> Result<()> {
    let (fixture, mut policy) = fixture()?;
    let path = "crates/consumer/src/lib.rs";
    write(fixture.root(), path, "fn run(){ CreateWindowExW(); }")?;
    let sources = vec![path.into()];
    policy.boundaries.push(Boundary {
        owner: "consumer".into(),
        concern: "Native peer creation".into(),
        source_sha256: fingerprint(fixture.root(), &sources)?,
        sources,
        capabilities: vec![Capability::Window, Capability::Raster],
    });
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    policy.boundaries[0].capabilities = vec![Capability::Window];
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    policy.boundaries[0].owner = "host".into();
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    Ok(())
}

/// Reject corrupt registries and fail closed on unsupported versions.
#[test]
fn policy_rejects_unknown_schema_duplicate_and_stale_records() -> Result<()> {
    let (fixture, mut policy) = fixture()?;
    policy.schema_version = 2;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    policy.schema_version = 1;
    policy
        .dependencies
        .push(Dependency { package: "egui".into(), owners: vec!["consumer".into()] });
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    policy
        .dependencies
        .push(Dependency { package: "egui".into(), owners: vec!["consumer".into()] });
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    assert!(
        toml::from_str::<Policy>(
            "schema_version=1\nowner='host'\ndependencies=[]\nboundaries=[]\nallow_everything=true"
        )
        .is_err()
    );
    Ok(())
}

/// Even an otherwise registered native window owner cannot directly enter a modal native loop.
#[test]
fn modal_dialogs_require_separate_exact_boundaries() -> Result<()> {
    for source in [
        "fn run(){ events.run_return(|_,_,_| { GetOpenFileNameW(); }); }",
        "use windows::Win32::UI::Shell::FileOpenDialog as Dialog; fn run(){WindowBuilder::new();}",
        "macro_rules! modal { () => { MessageBoxW() }; } fn run(){events.run_return(callback);}",
        "unsafe extern \"system\" { #[link_name=\"TaskDialogIndirect\"] fn modal(); } fn run(){WindowBuilder::new();}",
    ] {
        let (fixture, mut policy) = fixture()?;
        let path = "crates/consumer/src/lib.rs";
        write(fixture.root(), path, source)?;
        let sources = vec![path.into()];
        policy.boundaries.push(Boundary {
            owner: "consumer".into(),
            concern: "Invalid mixed modal/window ownership".into(),
            source_sha256: fingerprint(fixture.root(), &sources)?,
            sources,
            capabilities: vec![Capability::Window, Capability::Modal],
        });
        assert!(
            check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err(),
            "accepted {source}"
        );
    }
    let (fixture, mut policy) = fixture()?;
    let path = "crates/consumer/src/dialog.rs";
    write(fixture.root(), path, "fn select(){ GetSaveFileNameW(); }")?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    let sources = vec![path.into()];
    policy.boundaries.push(Boundary {
        owner: "consumer".into(),
        concern: "Dedicated shell selection outside application callbacks".into(),
        source_sha256: fingerprint(fixture.root(), &sources)?,
        sources,
        capabilities: vec![Capability::Modal],
    });
    check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace)?;
    write(fixture.root(), path, "fn select(){ GetSaveFileNameW(); GetOpenFileNameW(); }")?;
    assert!(check(fixture.root(), &policy, &fixture.architecture, &fixture.workspace).is_err());
    Ok(())
}

/// Native COM aliases, FFI link names, inactive branches and macro tokens preserve modal authority.
#[test]
fn modal_capabilities_recognize_all_existing_native_entrypoints() -> Result<()> {
    for source in [
        "use windows::Win32::UI::Shell::IFileOpenDialog as Picker;",
        "macro_rules! modal { () => { GetOpenFileNameW() }; }",
        "#[cfg(test)] fn run(){ GetSaveFileNameW(); }",
        "#[cfg(target_os=\"windows\")] fn run(){ FileSaveDialog; }",
        "unsafe extern \"system\" { #[link_name=\"MessageBoxW\"] fn ordinary(); }",
    ] {
        assert_eq!(ui_source::findings(source, &[])?, BTreeSet::from([Capability::Modal]));
    }
    Ok(())
}
