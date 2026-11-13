// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual authenticated TCP, command matching, prepared grants and Windows file custody.
mod fixture;
mod rig;
use super::*;
use fixture::Source;
use rig::Rig;
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
};
use sugarsubstitute_contracts::execution::JobContext;

/// External worker actor; the composition thread admits and observes each finite native batch.
struct Peer<'a> {
    rig: &'a Rig,
    native: Arc<Mutex<Native>>,
    stop: Arc<AtomicBool>,
    pressure: Arc<AtomicBool>,
    peer: TcpStream,
    reader: BufReader<TcpStream>,
}
impl Peer<'_> {
    /// Submit the actual production batch without constructing another ordinary-capacity allowance.
    fn batch(&self) -> Result<Batch> {
        let native = self.native.clone();
        let stop = self.stop.clone();
        let pressure = self.pressure.clone();
        self.rig.storage("native_batch", 10 * 1024 * 1024, move |context| {
            service::run(native, stop, pressure, "native".into(), context)
        })
    }
    /// Act as the external worker; native storage effects remain inside the production service.
    fn request(&mut self, serial: u64, operation: serde_json::Value) -> Result<serde_json::Value> {
        writeln!(
            self.peer,
            "{}",
            serde_json::json!({"version":1,"event":"downloads","id":"native","sequence":serial,"data":{"version":1,"serial":serial,"operation":operation}})
        )?;
        // The request is already in the loopback socket; each finite batch yields to the executor
        // between short receive polls until the service has admitted this command.
        loop {
            assert_eq!(self.batch()?, Batch::Continue);
            if self.native.lock().map_err(|_| "Native custody poisoned")?.serial == serial {
                break;
            }
        }
        let mut reply = String::new();
        self.reader
            .by_ref()
            .take(provisioning_transport::MAX_FRAME_BYTES as u64)
            .read_line(&mut reply)?;
        if !reply.ends_with('\n') {
            return Err("Native reply missing its bounded delimiter".into());
        }
        let value: serde_json::Value = serde_json::from_str(&reply)?;
        assert_eq!(value["operation"], "downloads_reply");
        assert_eq!(value["input"]["serial"], serial);
        Ok(value["input"]["result"].clone())
    }
}

/// The complete native path preserves bytes, rejects escaped selectors and observes actual closure.
#[test]
fn authenticated_native_service_uses_only_the_prepared_grant_and_flushes_before_settlement()
-> Result<()> {
    let rig = Rig::new()?;
    let (mut transport, runner) = Transport::pair();
    let port = transport.take_downloads()?;
    let mut socket_job = rig.client.submit_async(
        &rig.scope,
        rig.spec(JobClass::IoOrdinary, provisioning_transport::RESERVATION_BYTES)?,
        move |context| runner.run(context),
    )?;
    let (transport, port_number, token) = rig.storage("listen", 4096, move |context| {
        loop {
            match transport.receive(&context)? {
                Some(TransportEvent::Listening { port, token }) => {
                    return Ok((transport, port, token));
                }
                Some(_) => return Err(io::Error::other("Unexpected native startup frame")),
                None => {}
            }
        }
    })?;
    let credential = token.clone();
    let source = rig.storage("source", 32 * 1024 * 1024, move |context| Source::new(&context))?;
    let prepared = rig.storage("prepare", 16 * 1024 * 1024, move |context| {
        source.prepare(port_number, token, &context)
    })?;
    let native = Arc::new(Mutex::new(Native {
        port,
        access: Some(prepared.input.provisioning_downloads()?),
        active: BTreeSet::new(),
        serial: 0,
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let pressure = Arc::new(AtomicBool::new(false));
    let mut peer = TcpStream::connect(("127.0.0.1", port_number))?;
    peer.set_nodelay(true)?;
    writeln!(
        peer,
        "{}",
        serde_json::json!({"version":1,"event":"authenticate","token":credential})
    )?;
    let reader = BufReader::new(peer.try_clone()?);
    let mut actor = Peer { rig: &rig, native, stop, pressure, peer, reader };
    assert_eq!(actor.batch()?, Batch::Continue);
    let entry = actor.request(1, serde_json::json!({"op":"entry","key":"native qualification"}))?;
    assert_eq!(entry["ok"], true);
    let path = PathBuf::from(entry["data"]["path"].as_str().ok_or("Native entry path absent")?);
    let folder =
        path.file_name().and_then(|name| name.to_str()).ok_or("Native entry name absent")?;
    let name = format!("{folder}/payload");
    let opened =
        actor.request(2, serde_json::json!({"op":"open","name":name,"expected":1_048_579}))?;
    assert_eq!(opened["ok"], true);
    let held = path.clone();
    let external = prepared.root.path().join("authored-directory");
    rig.storage("native_custody_and_junction", 1024 * 1024, move |_| {
        assert!(std::fs::rename(held.join("payload"), held.join("replaced")).is_err());
        assert!(std::fs::write(held.join("payload"), b"replaced").is_err());
        fixture::junction(&held.join("linked"), &external)
    })?;
    let bytes: Vec<_> = (0..1_048_579).map(|index| (index % 251) as u8).collect();
    let mut serial = 3;
    for (index, chunk) in bytes.chunks(24 * 1024).enumerate() {
        let hex: String = chunk.iter().map(|byte| format!("{byte:02x}")).collect();
        let result = actor.request(
            serial,
            serde_json::json!({"op":"chunk","name":name,"offset":index*24*1024,"hex":hex}),
        )?;
        assert_eq!(result["ok"], true);
        serial += 1;
    }
    let denied =
        actor.request(serial, serde_json::json!({"op":"write","name":"../authored","hex":"00"}))?;
    assert_eq!(denied["ok"], false);
    serial += 1;
    assert_eq!(
        actor.request(
            serial,
            serde_json::json!({"op":"write","name":format!("{folder}/linked/keep"),"hex":"626164"})
        )?["ok"],
        false
    );
    serial += 1;
    for _ in 0..2 {
        let rejected = actor.request(
            serial,
            serde_json::json!({"op":"open","name":format!("{folder}/oversized"),"expected":24_u64*1024*1024*1024}),
        )?;
        assert_eq!(rejected["error"]["code"], "ENOSPC");
        serial += 1;
    }
    assert!(actor.pressure.load(Ordering::Acquire));
    actor.stop.store(true, Ordering::Release);
    assert_eq!(actor.batch()?, Batch::Stopped);
    assert!(actor.native.lock().map_err(|_| "Native custody poisoned")?.active.is_empty());
    actor.peer.shutdown(Shutdown::Both)?;
    drop(actor);
    assert!(matches!(rig.completion(&mut socket_job)?.outcome, JobOutcome::Success(())));
    drop(transport);
    let expected = Sha256::digest(&bytes);
    drop(bytes);
    rig.storage("verify", 4 * 1024 * 1024, move |context| {
        let _memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
        assert_eq!(Sha256::digest(std::fs::read(path.join("payload"))?), expected);
        assert_eq!(std::fs::read(prepared.root.path().join("authored"))?, b"preserve");
        assert_eq!(
            std::fs::read(prepared.root.path().join("authored-directory/keep"))?,
            b"preserve"
        );
        assert!(!path.join("oversized").exists());
        drop(prepared.input);
        drop(prepared.cache);
        prepared.root.close()?;
        Ok(())
    })
}
