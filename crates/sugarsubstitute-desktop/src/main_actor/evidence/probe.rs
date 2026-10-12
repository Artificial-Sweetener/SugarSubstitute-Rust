// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample actual installed owner exports in the native selected top execution context.

use super::*;

/// Module import consumes the installed owner, without running setup or registering another extension.
/// `_setupDone` is the inspected 0.15.0 SugarCubes UI's completed async setup observation.
const SAMPLE: &str = r#"(async()=>{
const app=window.comfyAPI?.app?.app;
if(!app?.isGraphReady||app.vueAppReady!==true||!app.positionConversion||!window.SugarCubes||
 !Array.isArray(app.extensions)||!app.extensions.some(extension=>extension.name==='SugarCubes.UI'))return 'null';
const cubes=await import('/extensions/SugarCubes/comfyui/ui.js');
const canvas=app?.canvas?.canvas;
const bounds=canvas?.getBoundingClientRect();
const registered=Array.isArray(app?.extensions)&&app.extensions.includes(cubes.sugarCubesExtension);
const graph=!!app?.graph&&app.canvas?.graph===app.graph;
const connected=!!canvas?.isConnected&&!!bounds&&bounds.width>0&&bounds.height>0;
const loading=document.readyState==='complete'&&!app?.configuringGraph&&
 !Array.from(document.querySelectorAll('[data-testid="loading-overlay"],.p-progressspinner')).some(el=>
  el.isConnected&&el.getBoundingClientRect().width>0&&getComputedStyle(el).visibility!=='hidden');
const ready=cubes.sugarCubesUI?._setupDone===true&&!!window.SugarCubes&&
 typeof window.SugarCubes.openLibrary==='function';
if(!(graph&&connected&&loading&&registered&&ready))return 'null';
return JSON.stringify({schema_version:1,launch_id:LAUNCH,navigation_generation:NAV,
 outcome:{status:'ready',evidence:{frontend_version:FRONT,sugarcubes_version:CUBES,
 app_graph_ready:graph,canvas_connected:connected,loading_finished:loading,
 sugarcubes_registered:registered,sugarcubes_ready:ready}}});
})()"#;

pub(in crate::main_actor) fn probe_parameters(
    target: ProbeTarget,
    request: &MainWindowRequest,
) -> Result<String, DesktopError> {
    let frontend = serde_json::to_string(request.versions().frontend())
        .map_err(|cause| DesktopError::new("encode_main_versions", cause))?;
    let cubes = serde_json::to_string(request.versions().sugarcubes())
        .map_err(|cause| DesktopError::new("encode_main_versions", cause))?;
    let expression = SAMPLE
        .replace("LAUNCH", &target.identity.launch.get().to_string())
        .replace("NAV", &target.identity.navigation.get().to_string())
        .replace("FRONT", &frontend)
        .replace("CUBES", &cubes);
    let expression = serde_json::to_string(&expression)
        .map_err(|cause| DesktopError::new("encode_main_probe", cause))?;
    let parameters = format!(
        "{{\"expression\":{expression},\"contextId\":{},\"awaitPromise\":true,\"returnByValue\":true}}",
        target.context
    );
    if parameters.len() > 8192 {
        return Err(DesktopError::message("encode_main_probe", "probe bound exceeded"));
    }
    Ok(parameters)
}
