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
//! Connect the local shell to GIMP's bounded private stdio transport independently of Tauri.

mod frames;
mod state;
pub use frames::NativeLimits;
use serde::Deserialize;
use serde_json::Value;
use state::State;
use std::{
    io::{BufRead, Write},
    sync::{Arc, Mutex, mpsc::sync_channel},
    thread,
    time::Duration,
};

type Shared = Arc<Mutex<State>>;
type Disconnected = Arc<dyn Fn(String) + Send + Sync>;

/// Scope opaque native primitive calls to one GIMP parent process.
#[derive(Clone)]
pub struct NativeLink {
    state: Shared,
    disconnected: Disconnected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    #[serde(rename = "type")]
    kind: String,
    id: u32,
    result: Value,
}

impl NativeLink {
    /// Start bounded IO workers; process shutdown closes their borrowed standard handles.
    pub fn start(
        input: impl BufRead + Send + 'static,
        output: impl Write + Send + 'static,
        limits: NativeLimits,
        disconnected: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        if limits.frame_bytes < 1 || limits.pending_requests < 1 || limits.pending_requests > 64 {
            return Err("Invalid native transport limits.".into());
        }
        let (writer, outgoing) = sync_channel::<Vec<u8>>(limits.pending_requests * 2);
        let state = Arc::new(Mutex::new(State {
            limits,
            writer: Some(writer),
            calls: Default::default(),
            next: 0,
            charged: 0,
            closed: false,
        }));
        let disconnected: Disconnected = Arc::new(disconnected);
        let write_state = state.clone();
        let write_disconnected = disconnected.clone();
        thread::Builder::new()
            .name("GIMP native writer".into())
            .spawn(move || {
                let mut output = output;
                while let Ok(frame) = outgoing.recv() {
                    let result = output.write_all(&frame).and_then(|()| output.flush());
                    if let Ok(mut state) = write_state.lock() {
                        state.charged = state.charged.saturating_sub(frame.len());
                    }
                    if let Err(error) = result {
                        disconnect(
                            &write_state,
                            &write_disconnected,
                            format!("Write GIMP native pipe: {error}"),
                        );
                        break;
                    }
                }
            })
            .map_err(|error| format!("Start native writer: {error}"))?;
        let read_state = state.clone();
        let read_disconnected = disconnected.clone();
        if let Err(error) = thread::Builder::new()
            .name("GIMP native reader".into())
            .spawn(move || {
                if let Err(reason) = read_replies(input, &read_state, limits.frame_bytes) {
                    disconnect(&read_state, &read_disconnected, reason);
                }
            })
        {
            disconnect(
                &state,
                &disconnected,
                format!("Start native reader: {error}"),
            );
            return Err("The GIMP native reader could not start.".into());
        }
        Ok(Self {
            state,
            disconnected,
        })
    }

    /// Submit on the command boundary before waiting asynchronously for its correlated reply.
    pub fn submit(&self, command: Value) -> Result<u32, String> {
        self.state
            .lock()
            .map_err(|_| "The native state is unavailable.")?
            .submit(command)
    }

    /// Wait once; a timeout cancels native preparation and never replays the request.
    pub fn wait(&self, id: u32, timeout: Duration) -> Result<Value, String> {
        let receiver = self
            .state
            .lock()
            .map_err(|_| "The native state is unavailable.")?
            .calls
            .get_mut(&id)
            .and_then(|call| call.receiver.take())
            .ok_or("The native request is unavailable or already being awaited.")?;
        let result = receiver
            .recv_timeout(timeout)
            .map_err(|_| "The native request timed out or disconnected.".to_owned())
            .and_then(|result| result);
        if result.is_err() {
            self.cancel(id)?;
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The native state is unavailable.")?;
        if state
            .calls
            .get(&id)
            .is_some_and(|call| call.snapshot.is_none() || call.cancelled)
        {
            state.calls.remove(&id);
        }
        result
    }

    /// Cancel queued preparation or a completed offscreen capture without undoing committed output.
    pub fn cancel(&self, id: u32) -> Result<(), String> {
        let result = self
            .state
            .lock()
            .map_err(|_| "The native state is unavailable.")?
            .cancel(id);
        if let Err(reason) = &result {
            self.close(reason.clone());
        }
        result
    }

    /// Retire a renderer's transient requests while preserving the parent's document and request frontier.
    pub fn reset(&self) -> Result<(), String> {
        let result = self
            .state
            .lock()
            .map_err(|_| "The native state is unavailable.")?
            .reset();
        if let Err(reason) = &result {
            self.close(reason.clone());
        }
        result
    }

    /// End this parent lifetime and reject every pending renderer wait once.
    pub fn close(&self, reason: String) {
        disconnect(&self.state, &self.disconnected, reason);
    }
}

/// Decode only correlation here; Python and TypeScript own primitive schema validation.
fn read_replies(mut input: impl BufRead, state: &Shared, maximum: usize) -> Result<(), String> {
    while let Some(frame) = frames::read_frame(&mut input, maximum)? {
        let reply: Reply = serde_json::from_slice(&frame)
            .map_err(|error| format!("Decode GIMP native reply: {error}"))?;
        if reply.kind != "reply" || reply.id < 1 || reply.id > i32::MAX as u32 {
            return Err("The GIMP parent sent an invalid reply envelope.".into());
        }
        state
            .lock()
            .map_err(|_| "The native state is unavailable.")?
            .receive(reply.id, reply.result);
    }
    Err("The GIMP parent disconnected.".into())
}

/// Drain response ownership before notifying the Tauri process lifecycle owner.
fn disconnect(state: &Shared, callback: &Disconnected, reason: String) {
    let Ok(mut state) = state.lock() else {
        callback("The GIMP native state became unavailable.".into());
        return;
    };
    if state.closed {
        return;
    }
    state.closed = true;
    state.writer = None;
    for (_, mut call) in std::mem::take(&mut state.calls) {
        if let Some(reply) = call.reply.take() {
            let _ = reply.send(Err(reason.clone()));
        }
    }
    drop(state);
    callback(reason);
}
