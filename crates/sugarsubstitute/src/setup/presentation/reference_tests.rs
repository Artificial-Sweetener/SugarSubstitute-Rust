// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare native peer rectangles with offscreen measurements of the authored installer.
//! The fixture retains normal/compact one-times-scale geometry captured before UI replacement.
use super::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Reference {
    viewport: [f32; 4],
    controls: BTreeMap<String, [f32; 4]>,
}
struct Comparison {
    reference: BTreeMap<String, Reference>,
    resources: SceneResources,
    differences: Vec<String>,
}
impl Comparison {
    /// Both window sizes exercise the same live application page and native composition function.
    fn check(&mut self, name: &str, flow: &SetupFlow, now: Duration) -> Result {
        for (width, height, suffix) in [(1180, 760, ""), (720, 560, "-compact")] {
            let key = format!("{name}{suffix}");
            let reference = self.reference.get(&key).ok_or("Reference absent")?;
            let mut view = view(width, height, 1.0);
            view.now = now;
            let scene = compose_flow(flow, view, &mut self.resources, None)?;
            if let Err(cause) = compare(&key, 0, scene.layout.viewport, reference.viewport) {
                self.differences.push(cause.to_string());
            }
            let mut observed = BTreeMap::new();
            for control in &scene.layout.shell_controls {
                observed.insert(control.id.get().to_string(), control.bounds);
            }
            for control in &scene.layout.page_controls {
                let mut bounds = control.bounds;
                bounds.x += scene.layout.viewport.x;
                bounds.y += scene.layout.viewport.y;
                observed.insert(control.id.get().to_string(), bounds);
            }
            assert_eq!(observed.len(), reference.controls.len(), "{key} native peer inventory");
            for (id, expected) in &reference.controls {
                if let Err(cause) = compare(
                    &key,
                    id.parse()?,
                    *observed.get(id).ok_or("Original control absent")?,
                    *expected,
                ) {
                    self.differences.push(cause.to_string());
                }
            }
        }
        Ok(())
    }
}

/// CSS quantizes layout to sixty-fourths of a logical pixel; native metrics retain full precision.
fn compare(page: &str, id: u16, actual: Bounds, expected: [f32; 4]) -> Result {
    for (actual, expected) in
        [actual.x, actual.y, actual.width, actual.height].into_iter().zip(expected)
    {
        if (actual - expected).abs() > 0.1 {
            return Err(format!("{page} peer {id}: native {actual}, reference {expected}").into());
        }
    }
    Ok(())
}

#[test]
fn all_original_page_controls_keep_their_normal_and_compact_placement() -> Result {
    let mut comparison = Comparison {
        reference: serde_json::from_str(include_str!("layout_reference.json"))?,
        resources: SceneResources::new("en")?,
        differences: Vec::new(),
    };
    let mut flow = SetupFlow::new("D:\\Apps\\SugarSubstitute".into())?;
    comparison.check("language", &flow, Duration::ZERO)?;
    let request = primary(&mut flow)?;
    receive(&mut flow, request, Completed::Locale)?;
    comparison.check("location", &flow, Duration::ZERO)?;
    let request = primary(&mut flow)?;
    receive(
        &mut flow,
        request,
        Completed::Location(Location {
            comfy_root: "D:\\Test\\ComfyUI".into(),
            models: "D:\\Test\\models".into(),
            outputs: "D:\\Test\\outputs".into(),
            existing: None,
        }),
    )?;
    comparison.check("runtime", &flow, Duration::ZERO)?;
    flow.set_mode(RuntimeMode::Attached)?;
    assert!(flow.primary(Duration::ZERO)?.is_none());
    comparison.check("configuration-existing", &flow, Duration::ZERO)?;
    flow.back()?;
    flow.set_mode(RuntimeMode::Managed)?;
    let request = primary(&mut flow)?;
    let request = options(&mut flow, request, "Stable")?.ok_or("Version request absent")?;
    let request = options(&mut flow, request, "v0.42.1")?.ok_or("Environment request absent")?;
    receive(
        &mut flow,
        request,
        Completed::Options(vec![Choice {
            value: "choice".into(),
            label: "NVIDIA".into(),
            description: "ComfyUI 0.42.1 · Python 3.13.12 · 2256 MB".into(),
            recommended: true,
            selection_token: Some("issued-by-test-boundary".into()),
        }]),
    )?;
    comparison.check("configuration", &flow, Duration::ZERO)?;
    let request = primary(&mut flow)?;
    receive(&mut flow, request, Completed::Validated)?;
    comparison.check("folders", &flow, Duration::ZERO)?;
    let request = primary(&mut flow)?;
    let install =
        receive(&mut flow, request, Completed::Validated)?.ok_or("Install request absent")?;
    flow.progress(install.id, "Installing Python dependencies".into(), Some(42.0))?;
    comparison.check("progress-collapsed", &flow, Duration::ZERO)?;
    flow.progress(install.id, "Next phase".into(), Some(62.0))?;
    flow.set_details(true);
    comparison.check("progress", &flow, Duration::from_secs(121))?;
    flow.set_details(false);
    comparison.check("progress-wait", &flow, Duration::from_secs(121))?;
    receive(&mut flow, install, Completed::Installed)?;
    comparison.check("complete", &flow, Duration::ZERO)?;
    for entry_missing in [false, true] {
        let mut flow = SetupFlow::new("D:\\Existing\\Substitute".into())?;
        inspect(
            &mut flow,
            Some(ExistingInstallation {
                install_root: "D:\\Existing\\Substitute".into(),
                comfy_root: "D:\\Saved\\ComfyUI".into(),
                executable_missing: true,
                entry_missing,
            }),
        )?;
        comparison.check(
            if entry_missing { "maintenance-recovery" } else { "maintenance" },
            &flow,
            Duration::ZERO,
        )?;
    }
    if comparison.differences.is_empty() {
        Ok(())
    } else {
        Err(comparison.differences.join("\n").into())
    }
}
