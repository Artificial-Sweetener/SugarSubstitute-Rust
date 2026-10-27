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
//! Prove companion endpoint admission and readiness checks without opening a native window.
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
use sugarcubes_gimp_companion::connection::ComfyEndpoint;

#[path = "../../../integrations/gimp/companion/src/infrastructure/probe.rs"]
mod probe;

#[cfg(all(windows, feature = "desktop"))]
#[test]
/// Keep the disconnected shell embedded after temporary build inputs are removed.
fn embeds_the_offline_shell_in_the_native_context() -> Result<(), Box<dyn std::error::Error>> {
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
    for name in [
        "index.html",
        "main.js",
        "shell.css",
        "palette.css",
        "LICENSE.txt",
    ] {
        let asset = context
            .assets()
            .get(&name.into())
            .ok_or_else(|| format!("Missing embedded asset: {name}"))?;
        assert!(!asset.is_empty(), "{name}");
    }
    Ok(())
}

#[cfg(all(windows, feature = "desktop"))]
#[test]
/// Preserve the native shell's declared sizing when window creation moves behind parent admission.
fn retains_the_companion_window_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let window = context
        .config()
        .app
        .windows
        .first()
        .ok_or("Missing companion window")?;
    assert_eq!(window.label, "companion");
    assert_eq!(window.title, "SugarCubes — GIMP");
    assert_eq!((window.width, window.height), (520.0, 760.0));
    assert_eq!(
        (window.min_width, window.min_height),
        (Some(304.0), Some(240.0))
    );
    assert!(window.resizable && window.decorations);
    Ok(())
}

#[test]
/// Preserve explicit local ports across validation and canonical navigation.
fn admits_explicit_loopback_ports_and_canonical_origins() -> Result<(), Box<dyn std::error::Error>>
{
    let endpoint = ComfyEndpoint::parse("http://127.0.0.1:8226")?;
    assert_eq!(endpoint.address(), "http://127.0.0.1:8226/");
    assert_eq!(endpoint.socket().port(), 8226);
    assert!(ComfyEndpoint::parse("http://localhost:8188/").is_ok());
    assert!(ComfyEndpoint::parse("http://[::1]:8188/").is_ok());
    Ok(())
}

#[test]
/// Reject native destinations outside the configured local prototype boundary.
fn rejects_unconfigured_networks_credentials_and_url_payloads() {
    for address in [
        "https://127.0.0.1:8188/",
        "http://example.com/",
        "http://192.168.1.2/",
        "http://user:secret@localhost/",
        "file:///C:/private",
        "http://localhost/?command=start",
        "http://localhost/#fragment",
        "http://localhost/other-path",
    ] {
        assert!(ComfyEndpoint::parse(address).is_err(), "{address}");
    }
}

#[test]
/// Distinguish ComfyUI readiness from arbitrary JSON or an unrelated open port.
fn requires_comfy_readiness_shape_and_bounded_json() {
    assert!(ComfyEndpoint::accepts_readiness(
        br#"{"system":{},"devices":[]}"#
    ));
    for body in [
        b"<html>Other application</html>".as_slice(),
        b"{}",
        b"null",
        b"[]",
    ] {
        assert!(!ComfyEndpoint::accepts_readiness(body));
    }
    assert!(!ComfyEndpoint::accepts_readiness(&vec![b' '; 65537]));
}

/// Exercise the production native boundary against a real isolated loopback response.
fn probe_response(response: Vec<u8>) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let endpoint = ComfyEndpoint::parse(&format!("http://{}/", listener.local_addr()?))?;
    let server = thread::spawn(move || -> Result<String, std::io::Error> {
        let (mut stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while request.len() < 4096 && !request.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte)? == 0 {
                break;
            }
            request.push(byte[0]);
        }
        stream.write_all(&response)?;
        Ok(String::from_utf8_lossy(&request).into_owned())
    });
    let ready = probe::ready(&endpoint).is_ok();
    let request = server
        .join()
        .map_err(|_| "The readiness fixture worker failed.")??;
    Ok((ready, request))
}

#[test]
/// Exercise the actual native request independently of Tauri and a user's running ComfyUI.
fn probes_the_existing_comfy_readiness_route() -> Result<(), Box<dyn std::error::Error>> {
    let (ready, request) =
        probe_response(b"HTTP/1.0 200 OK\r\n\r\n{\"system\":{},\"devices\":[]}".to_vec())?;
    assert!(ready);
    assert!(request.starts_with("GET /system_stats HTTP/1.0\r\nHost: 127.0.0.1:"));
    assert!(request.ends_with("Connection: close\r\n\r\n"));
    Ok(())
}

#[test]
/// Fail closed on unsuccessful, malformed and unbounded server responses.
fn rejects_other_servers_and_oversized_native_responses() -> Result<(), Box<dyn std::error::Error>>
{
    for response in [
        b"HTTP/1.0 404 Not Found\r\n\r\n{\"system\":{},\"devices\":[]}".to_vec(),
        b"HTTP/1.0 200 OK\r\n\r\n<html>Other server</html>".to_vec(),
        b"Malformed HTTP response".to_vec(),
        vec![b' '; 66000],
    ] {
        assert!(!probe_response(response)?.0);
    }
    Ok(())
}
