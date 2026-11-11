// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve finite native batches, yielding ordinary capacity without releasing command custody.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;

/// Run only through the existing execution adapter; no thread, pool or scheduler is constructed.
pub(super) fn run(
    native: Arc<Mutex<Native>>,
    stop: Arc<AtomicBool>,
    pressure: Arc<AtomicBool>,
    request: String,
    context: JobContext,
) -> io::Result<Batch> {
    let _buffers = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    let mut native = native
        .try_lock()
        .map_err(|_| io::Error::other("Native download custody busy or poisoned"))?;
    let outcome = serve(&mut native, &stop, &pressure, &request, &context);
    if matches!(outcome, Ok(Batch::Continue)) {
        return outcome;
    }
    let Native { access, active, .. } = &mut *native;
    let access =
        access.as_ref().ok_or_else(|| io::Error::other("Prepared download grant absent"))?;
    let mut failures = Vec::new();
    active.retain(|name| match access.close(name) {
        Ok(()) => false,
        Err(cause) => {
            failures.push(cause);
            true
        }
    });
    if failures.is_empty() {
        return outcome;
    }
    Err(io::Error::other(CleanupFailure { primary: outcome.err(), failures }))
}
/// Preserve both the original service cause and every failed actual prefix flush.
#[derive(Debug)]
struct CleanupFailure {
    primary: Option<io::Error>,
    failures: Vec<io::Error>,
}
impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Native download service could not flush {} retained writers",
            self.failures.len()
        )
    }
}
impl std::error::Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.primary
            .as_ref()
            .or_else(|| self.failures.first())
            .map(|cause| cause as &dyn std::error::Error)
    }
}
/// Native replies bypass UI polling, while command identity, cancellation and original limits remain.
fn serve(
    native: &mut Native,
    stop: &AtomicBool,
    pressure: &AtomicBool,
    request: &str,
    context: &JobContext,
) -> io::Result<Batch> {
    let Native { port, access, active, serial } = native;
    let access =
        access.as_ref().ok_or_else(|| io::Error::other("Prepared download grant absent"))?;
    for _ in 0..64 {
        if stop.load(Ordering::Acquire) {
            if port.pending() {
                return Err(io::Error::other("Worker terminated with queued native downloads"));
            }
            return Ok(Batch::Stopped);
        }
        let Some(frame) = port.receive(context)? else { return Ok(Batch::Continue) };
        if stop.load(Ordering::Acquire) {
            return Err(io::Error::other("Worker terminated during native downloads"));
        }
        let envelope: wire::Envelope =
            serde_json::from_str(frame.as_str()).map_err(io::Error::other)?;
        if envelope.version != 1
            || envelope.event != "downloads"
            || envelope.id != request
            || envelope.data.version != 1
            || envelope.sequence == 0
            || envelope.sequence > 9_007_199_254_740_991
            || envelope.data.serial > 9_007_199_254_740_991
            || serial.checked_add(1) != Some(envelope.data.serial)
        {
            return Err(wire::invalid());
        }
        envelope.data.validate()?;
        *serial = envelope.data.serial;
        let operation = context.bounded_until(
            context.now().checked_add(Duration::from_secs(30)).map_err(io::Error::other)?,
        );
        let result = match envelope.data.execute(access, active, &operation) {
            Ok(data) => serde_json::json!({"ok":true,"data":data}),
            Err(cause) => {
                let code = match cause.kind() {
                    io::ErrorKind::NotFound => "ENOENT",
                    io::ErrorKind::AlreadyExists => "EEXIST",
                    io::ErrorKind::PermissionDenied => "EACCES",
                    io::ErrorKind::WouldBlock => "EBUSY",
                    io::ErrorKind::StorageFull => {
                        pressure.store(true, Ordering::Release);
                        "ENOSPC"
                    }
                    io::ErrorKind::Interrupted => "EINTR",
                    io::ErrorKind::TimedOut => "ETIMEDOUT",
                    _ => "EIO",
                };
                tracing::warn!(operation="provisioning_download_io", request, serial=*serial, cause=%cause, "Native download operation failed");
                serde_json::json!({"ok":false,"error":{"code":code,"osCode":cause.raw_os_error()}})
            }
        };
        let reply = serde_json::json!({"version":1,"id":request,"operation":"downloads_reply", "input":{"version":1,"serial":serial,"index":0,"done":true,"result":result}});
        port.send(serde_json::to_string(&reply).map_err(io::Error::other)?)?;
    }
    Ok(Batch::Continue)
}
