// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample actual installed owner exports in the native selected top execution context.

use super::*;

/// Module import consumes the installed owner, without running setup or registering another extension.
/// `_setupDone` observes completion of the SugarCubes UI owner's async setup.
const SAMPLE: &str = r#"(async()=>{
const app=window.comfyAPI?.app?.app;
let missing=0;
const check=(condition,bit)=>{if(!condition)missing|=bit;return !!condition;};
check(app?.isGraphReady,1);
check(app?.vueAppReady===true,2);
check(app?.positionConversion,4);
const publicApi=window.SugarCubes;
check(publicApi,8);
const extensions=app?.extensions;
const extensionArray=check(Array.isArray(extensions),16);
check(extensionArray&&extensions.some(extension=>extension.name==='SugarCubes.UI'),32);
if(missing)return JSON.stringify({incomplete:missing});
const cubes=await import('/extensions/SugarCubes/comfyui/ui.js');
const canvas=app?.canvas?.canvas;
const bounds=canvas?.getBoundingClientRect();
const registered=check(Array.isArray(app?.extensions)&&app.extensions.includes(cubes.sugarCubesExtension),64);
const graph=check(!!app?.graph&&app.canvas?.graph===app.graph,128);
const connected=check(!!canvas?.isConnected&&!!bounds&&bounds.width>0&&bounds.height>0,256);
const complete=check(document.readyState==='complete',512);
const configured=check(!app?.configuringGraph,1024);
const splash=check(!document.getElementById('splash-loader'),2048);
const loading=complete&&configured&&splash;
const setup=check(cubes.sugarCubesUI?._setupDone===true,4096);
const readyApi=window.SugarCubes;
const api=check(readyApi,16384);
const library=check(!!readyApi&&typeof readyApi.openLibrary==='function',8192);
const ready=setup&&api&&library;
if(missing)return JSON.stringify({incomplete:missing});
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
