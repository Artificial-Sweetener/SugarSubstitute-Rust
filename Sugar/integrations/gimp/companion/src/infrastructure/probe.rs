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
//! Read a bounded loopback ComfyUI response without blocking the native event thread.
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};
use sugarcubes_gimp_companion::connection::ComfyEndpoint;

/// Require ComfyUI's existing readiness shape within fixed time and response budgets.
pub(super) fn ready(
    endpoint: &ComfyEndpoint,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let timeout = Duration::from_secs(3);
    let deadline = Instant::now() + timeout;
    let mut connection = TcpStream::connect_timeout(&endpoint.socket(), timeout)?;
    connection.set_read_timeout(Some(timeout))?;
    connection.set_write_timeout(Some(timeout))?;
    let request = format!(
        "GET /system_stats HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        endpoint.socket()
    );
    connection.write_all(request.as_bytes())?;
    let mut response = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("The readiness request exceeded its time budget.")?;
        connection.set_read_timeout(Some(remaining))?;
        let received = connection.read(&mut chunk)?;
        if received == 0 {
            break;
        }
        if response.len() + received > 65536 {
            return Err("The readiness response exceeded its budget.".into());
        }
        response.extend_from_slice(&chunk[..received]);
    }
    let text = std::str::from_utf8(&response)?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .ok_or("Invalid readiness response.")?;
    if !(headers.starts_with("HTTP/1.0 200 ") || headers.starts_with("HTTP/1.1 200 "))
        || !ComfyEndpoint::accepts_readiness(body.as_bytes())
    {
        return Err("The configured server did not return ComfyUI readiness.".into());
    }
    Ok(())
}
