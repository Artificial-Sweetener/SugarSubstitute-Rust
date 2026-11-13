// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify measured native pages against real application navigation and source geometry.
use super::*;
use sugarsubstitute_application::setup_flow::{
    Choice, Completed, ExistingInstallation, Location, Outcome, Request,
};
use sugarsubstitute_contracts::{configuration::Theme, localization::LOCALES};
use sugarsubstitute_ui::{form::Role, scene::SceneCursor};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "reference_tests.rs"]
mod reference;
#[path = "render_tests.rs"]
mod render;

fn view(width: u32, height: u32, scale: f32) -> View {
    View {
        surface: PaintSpec {
            theme: Theme::Dark,
            scale,
            width: (width as f32 * scale).round() as u32,
            height: (height as f32 * scale).round() as u32,
        },
        scroll: 0.0,
        now: Duration::ZERO,
        interaction: Interaction::default(),
        text_revisions: [0; 5],
        accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    }
}
fn primary(flow: &mut SetupFlow) -> Result<Request> {
    flow.primary(Duration::ZERO)?.ok_or_else(|| "Expected native setup request".into())
}
fn receive(flow: &mut SetupFlow, request: Request, result: Completed) -> Result<Option<Request>> {
    Ok(flow.receive(request.id, Outcome::Completed(result), Duration::ZERO)?)
}
fn inspect(flow: &mut SetupFlow, existing: Option<ExistingInstallation>) -> Result {
    let request = primary(flow)?;
    receive(flow, request, Completed::Locale)?;
    let request = primary(flow)?;
    receive(
        flow,
        request,
        Completed::Location(Location {
            comfy_root: "E:\\ComfyUI".into(),
            models: "E:\\Models".into(),
            outputs: "E:\\Outputs".into(),
            existing,
        }),
    )?;
    Ok(())
}
fn options(flow: &mut SetupFlow, request: Request, label: &str) -> Result<Option<Request>> {
    receive(
        flow,
        request,
        Completed::Options(vec![Choice {
            value: "opaque".into(),
            label: label.into(),
            description: "An upstream environment description".into(),
            recommended: true,
            selection_token: Some("opaque-provisioning-token".into()),
        }]),
    )
}
fn check(flow: &SetupFlow, resources: &mut SceneResources) -> Result {
    for (width, height) in [(1180, 760), (720, 560)] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let view = view(width, height, scale);
            let scene = compose_flow(flow, view, resources, None)?;
            assert!(scene.layout.content_height > 0.0 && scene.layout.viewport.height > 0.0);
            if scene.layout.content_height > scene.layout.viewport.height {
                assert_eq!(
                    scene.layout.page_width,
                    scene.layout.viewport.width - sugarsubstitute_ui::form::SCROLL_GUTTER
                );
                let mut scrolled_view = view;
                scrolled_view.scroll = f32::MAX;
                let scrolled = compose_flow(flow, scrolled_view, resources, None)?;
                assert_eq!(
                    scrolled.layout.scroll,
                    scrolled.layout.content_height - scrolled.layout.viewport.height
                );
                for (before, after) in
                    scene.layout.page_controls.iter().zip(&scrolled.layout.page_controls)
                {
                    assert_eq!(before.id, after.id);
                    assert!(
                        (before.bounds.y - after.bounds.y - scrolled.layout.scroll).abs() < 0.001
                    );
                }
                SceneCursor::new(view.surface, view.accent, scrolled.page, None)?;
            } else {
                assert_eq!(scene.layout.page_width, scene.layout.viewport.width);
            }
            let mut ids = std::collections::BTreeSet::new();
            for control in scene.layout.shell_controls.iter().chain(&scene.layout.page_controls) {
                assert!(ids.insert(control.id));
                assert_ne!(
                    control.id.get(),
                    sugarsubstitute_desktop::ConsoleView::CONTROL_ID,
                    "Every page must coexist with the same retained native transcript"
                );
                assert!(control.bounds.width > 0.0 && control.bounds.height > 0.0);
                if !flow.can_edit() && !matches!(control.id.get(), 1 | 2 | 3 | 21) {
                    assert!(!control.availability.actionable());
                }
            }
            SceneCursor::new(view.surface, view.accent, scene.shell, None)?;
            SceneCursor::new(view.surface, view.accent, scene.page, None)?;
        }
    }
    Ok(())
}

#[test]
fn native_language_page_matches_authored_reference_geometry() -> Result {
    let flow = SetupFlow::new("E:\\Substitute".into())?;
    let mut resources = SceneResources::new("en")?;
    let scene = compose_flow(&flow, view(1180, 760, 1.0), &mut resources, None)?;
    assert_eq!(scene.layout.viewport, Bounds { x: 44.0, y: 153.0, width: 1092.0, height: 535.0 });
    assert!((scene.layout.content_height - 150.0).abs() < 0.01);
    let locale = scene
        .layout
        .page_controls
        .iter()
        .find(|peer| peer.id == Action::Locale.id())
        .ok_or("Locale peer missing")?;
    assert!((locale.bounds.x + scene.layout.viewport.x - 380.0).abs() < 0.01);
    assert!((locale.bounds.y + scene.layout.viewport.y - 442.5).abs() < 0.01);
    assert_eq!(locale.bounds.width, 420.0);
    let primary = scene
        .layout
        .shell_controls
        .iter()
        .find(|peer| peer.id == Action::Primary.id())
        .ok_or("Primary peer missing")?;
    assert_eq!(primary.bounds, Bounds { x: 968.0, y: 704.0, width: 168.0, height: 32.0 });
    Ok(())
}

#[test]
fn every_native_installation_page_composes_at_supported_scales_and_locales() -> Result {
    for locale in LOCALES {
        let mut resources = SceneResources::new(locale.id)?;
        let mut flow = SetupFlow::new("E:\\Substitute".into())?;
        let request = flow.set_locale(locale.id, Duration::ZERO)?;
        receive(&mut flow, request, Completed::Locale)?;
        check(&flow, &mut resources)?;
        let request = primary(&mut flow)?;
        receive(&mut flow, request, Completed::Locale)?;
        check(&flow, &mut resources)?;
        let request = primary(&mut flow)?;
        check(&flow, &mut resources)?;
        receive(
            &mut flow,
            request,
            Completed::Location(Location {
                comfy_root: "E:\\ComfyUI".into(),
                models: "E:\\Models".into(),
                outputs: "E:\\Outputs".into(),
                existing: None,
            }),
        )?;
        check(&flow, &mut resources)?;
        let request = primary(&mut flow)?;
        check(&flow, &mut resources)?;
        let request = options(&mut flow, request, "Stable")?.ok_or("Version request absent")?;
        let request =
            options(&mut flow, request, "Upstream version")?.ok_or("Environment request absent")?;
        options(&mut flow, request, "NVIDIA")?;
        check(&flow, &mut resources)?;
        let request = primary(&mut flow)?;
        receive(&mut flow, request, Completed::Validated)?;
        check(&flow, &mut resources)?;
        let request = primary(&mut flow)?;
        let request =
            receive(&mut flow, request, Completed::Validated)?.ok_or("Install request absent")?;
        flow.progress(request.id, "Installing an environment".into(), Some(40.0))?;
        check(&flow, &mut resources)?;
        flow.set_details(true);
        check(&flow, &mut resources)?;
        receive(&mut flow, request, Completed::Installed)?;
        check(&flow, &mut resources)?;
    }
    Ok(())
}

#[test]
fn attached_paths_and_maintenance_summaries_retain_original_unicode() -> Result {
    let mut resources = SceneResources::new("en")?;
    let mut flow = SetupFlow::new("E:\\用户\\Substitute".into())?;
    inspect(&mut flow, None)?;
    flow.set_mode(RuntimeMode::Attached)?;
    assert!(flow.primary(Duration::ZERO)?.is_none());
    let original = "E:\\用户\\🐈\\python.exe";
    flow.set_path(PathField::Python, original.into())?;
    check(&flow, &mut resources)?;
    let scene = compose_flow(&flow, view(720, 560, 1.0), &mut resources, None)?;
    let python = scene
        .layout
        .page_controls
        .iter()
        .find(|peer| peer.id == Action::Path(PathField::Python).id())
        .ok_or("Python field absent")?;
    assert_eq!(python.role, Role::TextField(original.into()));
    for missing in [false, true] {
        let mut flow = SetupFlow::new("E:\\Substitute".into())?;
        inspect(
            &mut flow,
            Some(ExistingInstallation {
                install_root: "E:\\Substitute".into(),
                comfy_root: "E:\\用户\\ComfyUI".into(),
                executable_missing: missing,
                entry_missing: missing,
            }),
        )?;
        check(&flow, &mut resources)?;
    }
    Ok(())
}

#[test]
fn waiting_failure_and_closing_keep_native_admission_and_feedback_consistent() -> Result {
    let mut resources = SceneResources::new("en")?;
    let mut flow = SetupFlow::new("E:\\Substitute".into())?;
    let request = primary(&mut flow)?;
    let mut waiting = view(720, 560, 1.0);
    let before = compose_flow(&flow, waiting, &mut resources, None)?;
    waiting.now = Duration::from_secs(120);
    let after = compose_flow(&flow, waiting, &mut resources, None)?;
    assert!(after.layout.viewport.height < before.layout.viewport.height);
    flow.receive(request.id, Outcome::Failed("A failure with retained input".into()), waiting.now)?;
    check(&flow, &mut resources)?;
    flow.closing();
    assert!(!flow.can_edit());
    let scene = compose_flow(&flow, waiting, &mut resources, None)?;
    assert!(!scene.layout.shell_controls.iter().any(|control| control.id == Action::Primary.id()));
    assert!(
        scene
            .layout
            .shell_controls
            .iter()
            .chain(&scene.layout.page_controls)
            .all(|control| !control.availability.actionable())
    );
    Ok(())
}

#[test]
fn native_scene_rejects_invalid_geometry_without_submitting_work() -> Result {
    let flow = SetupFlow::new("E:\\Substitute".into())?;
    let mut resources = SceneResources::new("en")?;
    for mut view in [view(719, 560, 1.0), view(720, 559, 1.0), view(720, 560, 1.0)] {
        if view.surface.width == 720 && view.surface.height == 560 {
            view.scroll = f32::NAN;
        }
        assert!(compose_flow(&flow, view, &mut resources, None).is_err());
    }
    Ok(())
}

#[test]
fn prepared_scene_retains_metadata_credit_until_its_values_retire() -> Result {
    use sugarsubstitute_contracts::execution::MemoryRelease;
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let flow = SetupFlow::new("E:\\Substitute".into())?;
    let mut resources = SceneResources::new("en")?;
    let short = MemoryLease::new(METADATA_BYTES - 1, Box::new(Release));
    assert!(compose_flow(&flow, view(1180, 760, 1.0), &mut resources, Some(&short)).is_err());
    let lease = MemoryLease::new(METADATA_BYTES + 1024 * 1024, Box::new(Release));
    let scene = compose_flow(&flow, view(1180, 760, 1.0), &mut resources, Some(&lease))?;
    assert!(lease.claim(1024 * 1024 + 1).is_err());
    assert!(lease.claim(1024 * 1024).is_ok());
    drop(scene);
    assert!(lease.claim(METADATA_BYTES + 1024 * 1024).is_ok());
    Ok(())
}

/// Snapshot admission is separate from the scene/raster budget exercised by each qualification.
fn compose_flow(
    flow: &SetupFlow,
    view: View,
    resources: &mut SceneResources,
    lease: Option<&MemoryLease>,
) -> io::Result<Scene> {
    struct Release;
    impl sugarsubstitute_contracts::execution::MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let budget = MemoryLease::new(SNAPSHOT_BYTES, Box::new(Release));
    let snapshot = Snapshot::capture(flow, view, &budget)?;
    super::compose(&snapshot, resources, lease)
}
