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
//! Exercise the production native pipe over bounded loopback sockets without a desktop window.

use serde_json::{Value, json};
use std::{
    error::Error,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::mpsc::{Receiver, channel},
    time::Duration,
};
use sugarcubes_gimp_companion::native::{NativeLimits, NativeLink};

type Proof = Result<(), Box<dyn Error>>;

struct Fixture {
    link: NativeLink,
    peer: BufReader<TcpStream>,
    closed: Receiver<String>,
}

impl Fixture {
    /// Supply real duplex IO and bounded test waits while retaining production correlation ownership.
    fn new() -> Result<Self, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let client = TcpStream::connect(listener.local_addr()?)?;
        let (peer, _) = listener.accept()?;
        peer.set_read_timeout(Some(Duration::from_secs(2)))?;
        peer.set_write_timeout(Some(Duration::from_secs(2)))?;
        let (closed, notification) = channel();
        let link = NativeLink::start(
            BufReader::new(client.try_clone()?),
            client,
            NativeLimits {
                frame_bytes: 512,
                pending_requests: 2,
            },
            move |reason| {
                let _ = closed.send(reason);
            },
        )?;
        Ok(Self {
            link,
            peer: BufReader::new(peer),
            closed: notification,
        })
    }

    /// Read an actual outgoing frame rather than inspecting private transport state.
    fn read(&mut self) -> Result<Value, Box<dyn Error>> {
        let mut frame = String::new();
        self.peer.read_line(&mut frame)?;
        Ok(serde_json::from_str(&frame)?)
    }

    /// Publish a native parent response without manufacturing a second Rust dispatcher.
    fn reply(&mut self, id: u32, result: Value) -> Proof {
        let mut frame = serde_json::to_vec(&json!({"type":"reply","id":id,"result":result}))?;
        frame.push(b'\n');
        self.peer.get_mut().write_all(&frame)?;
        self.peer.get_mut().flush()?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Closing the peer is the expected test teardown; a prior disconnect may already have closed it.
        let _ = self.peer.get_mut().shutdown(Shutdown::Both);
    }
}

#[test]
fn correlates_once_and_keeps_pending_calls_bounded() -> Proof {
    let mut f = Fixture::new()?;
    let first = f.link.submit(json!({"type":"context"}))?;
    let second = f.link.submit(json!({"type":"context"}))?;
    assert!(f.link.submit(json!({"type":"context"})).is_err());
    assert_eq!(f.read()?["id"], first);
    assert_eq!(f.read()?["id"], second);
    f.reply(second, json!({"kind":"second"}))?;
    f.reply(first, json!({"kind":"first"}))?;
    assert_eq!(f.link.wait(first, Duration::from_secs(2))?["kind"], "first");
    assert_eq!(
        f.link.wait(second, Duration::from_secs(2))?["kind"],
        "second"
    );
    assert!(f.link.wait(first, Duration::ZERO).is_err());
    assert!(f.link.submit(json!({"type":"context"}))? > second);
    Ok(())
}

#[test]
fn renderer_reset_cancels_retained_snapshots_without_reusing_ids() -> Proof {
    let mut f = Fixture::new()?;
    let snapshot = f.link.submit(json!({"type":"snapshot.begin"}))?;
    f.read()?;
    f.reply(snapshot, json!({"kind":"snapshot","token":"fixture"}))?;
    f.link.wait(snapshot, Duration::from_secs(2))?;
    f.link.reset()?;
    assert_eq!(f.read()?, json!({"type":"cancel","id":snapshot}));
    let next = f.link.submit(json!({"type":"context"}))?;
    assert!(next > snapshot);
    assert_eq!(f.read()?["id"], next);
    Ok(())
}

#[test]
fn explicit_release_retires_the_original_capture_ticket() -> Proof {
    let mut f = Fixture::new()?;
    let snapshot = f.link.submit(json!({"type":"snapshot.begin"}))?;
    f.read()?;
    f.reply(snapshot, json!({"kind":"snapshot","token":"fixture"}))?;
    f.link.wait(snapshot, Duration::from_secs(2))?;
    let release = f
        .link
        .submit(json!({"type":"snapshot.release","token":"fixture"}))?;
    f.read()?;
    f.reply(release, json!({"kind":"released"}))?;
    f.link.wait(release, Duration::from_secs(2))?;
    f.link.reset()?;
    let next = f.link.submit(json!({"type":"context"}))?;
    assert_eq!(f.read()?["id"], next);
    Ok(())
}

#[test]
fn a_timeout_sends_cancellation_without_replaying_the_command() -> Proof {
    let mut f = Fixture::new()?;
    let id = f.link.submit(json!({"type":"image.insert"}))?;
    assert_eq!(f.read()?["id"], id);
    assert!(f.link.wait(id, Duration::ZERO).is_err());
    assert_eq!(f.read()?, json!({"type":"cancel","id":id}));
    f.reply(id, json!({"kind":"failure","status":"cancelled"}))?;
    let next = f.link.submit(json!({"type":"context"}))?;
    assert!(next > id);
    assert_eq!(f.read()?["id"], next);
    Ok(())
}

#[test]
fn parent_eof_rejects_pending_waits_and_future_submissions() -> Proof {
    let mut f = Fixture::new()?;
    let id = f.link.submit(json!({"type":"context"}))?;
    f.read()?;
    f.peer.get_mut().shutdown(Shutdown::Both)?;
    f.closed.recv_timeout(Duration::from_secs(2))?;
    assert!(f.link.wait(id, Duration::ZERO).is_err());
    assert!(f.link.submit(json!({"type":"context"})).is_err());
    Ok(())
}

#[test]
fn malformed_and_oversized_parent_frames_end_the_link() -> Proof {
    for frame in [b"{bad json}\n".to_vec(), vec![b'x'; 513]] {
        let mut f = Fixture::new()?;
        f.peer.get_mut().write_all(&frame)?;
        f.closed.recv_timeout(Duration::from_secs(2))?;
        assert!(f.link.submit(json!({"type":"context"})).is_err());
    }
    Ok(())
}

#[test]
fn encoded_requests_are_bounded_before_queue_admission() -> Proof {
    let f = Fixture::new()?;
    assert!(
        f.link
            .submit(json!({"type":"image.insert","data":"x".repeat(513)}))
            .is_err()
    );
    assert!(f.link.submit(json!("not a command")).is_err());
    Ok(())
}

#[test]
fn packaged_limits_use_the_shared_canvas_budget() -> Proof {
    let limits = NativeLimits::packaged()?;
    assert_eq!(limits.pending_requests, 8);
    assert!(limits.frame_bytes > 64 * 1024 * 1024);
    Ok(())
}
