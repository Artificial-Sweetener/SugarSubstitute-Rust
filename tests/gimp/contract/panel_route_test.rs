//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Prove native transport admission independently of Tauri and the shared operation schemas.
use serde_json::json;
use sugarcubes_gimp_companion::{connection::ComfyEndpoint, panel::PanelRoute};
use url::Url;

#[test]
fn navigation_requires_exact_origin_and_current_identity() -> Result<(), String> {
    let endpoint =
        ComfyEndpoint::parse("http://127.0.0.1:32981/").map_err(|error| error.to_string())?;
    let route = PanelRoute::new(&endpoint, "fixture-session-0001".into())?;
    assert!(route.accepts_navigation(route.url()));
    for value in [
        "http://127.0.0.1:32981/",
        "http://127.0.0.1:8188/?sc-panel-host=ai.artificialsweetener.sugarcubes.gimp&sc-panel-session=fixture-session-0001&sc-panel-transport=native",
        "http://127.0.0.1:32981/?sc-panel-host=ai.artificialsweetener.sugarcubes.gimp&sc-panel-session=stale-session-0001&sc-panel-transport=native",
    ] {
        assert!(!route.accepts_navigation(&Url::parse(value).map_err(|error| error.to_string())?));
    }
    let mut duplicate = route.url().clone();
    duplicate
        .query_pairs_mut()
        .append_pair("sc-panel-session", route.session());
    assert!(!route.accepts_navigation(&duplicate));
    let history_url = Url::parse("http://127.0.0.1:32981/#current-workflow")
        .map_err(|error| error.to_string())?;
    assert!(route.accepts_origin(&history_url));
    assert!(!route.accepts_navigation(&history_url));
    assert!(
        !route.accepts_origin(
            &Url::parse("http://127.0.0.1:8188/#current-workflow")
                .map_err(|error| error.to_string())?
        )
    );
    Ok(())
}

#[test]
fn transport_refuses_private_native_operations_and_excessive_bytes() -> Result<(), String> {
    let endpoint =
        ComfyEndpoint::parse("http://localhost:32981/").map_err(|error| error.to_string())?;
    let route = PanelRoute::new(&endpoint, "fixture-session-0001".into())?;
    let message = json!({"protocol":"sugarcubes.canvas", "session":route.session(), "type":"read", "version":3});
    let size = serde_json::to_vec(&message)
        .map_err(|error| error.to_string())?
        .len();
    assert!(route.accepts_message(&message, size));
    assert!(!route.accepts_message(&message, size - 1));
    assert!(!route.accepts_message(
        &json!({"protocol":"native", "session":route.session(), "type":"snapshot.begin"}),
        4096
    ));
    assert!(!route.accepts_message(
        &json!({"protocol":"sugarcubes.canvas", "session":"stale-session-0001"}),
        4096
    ));
    Ok(())
}

#[test]
fn opaque_session_validation_has_no_partial_matches() -> Result<(), String> {
    let endpoint =
        ComfyEndpoint::parse("http://localhost:32981/").map_err(|error| error.to_string())?;
    for invalid in ["short", "fixture-session-0001\n", "fixture_session_0001"] {
        assert!(PanelRoute::new(&endpoint, invalid.into()).is_err());
    }
    Ok(())
}
