// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify a real connected loopback peer and transfer under the shared execution owner.

use super::*;
use std::{
    io::{self, Read},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobCompletion, JobId, JobOutcome,
    JobSpec, Moment, Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner,
    clock_fixture::ManualClock,
    http_fixture::{Exchange, Server},
};
use sugarsubstitute_supervision::{
    backend_listener::ConnectedLoopback,
    cache::{CachePolicy, utc_seconds},
};

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

// One authored black pixel: GIF89a, a two-color table and LZW clear/index/end codes.
const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";
const JPEG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sugarsubstitute-image-io/src/tests/jpeg/fixtures/444.jpg"
));
const JPEG_TARGET: &str = "/view?filename=image.jpg&subfolder=&type=output";

#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);

impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision = revision.saturating_add(1);
        self.1.notify_all();
        Ok(())
    }
}

fn take<T, E>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
    loop {
        let revision = *wake.0.lock().map_err(|_| "Wake poisoned")?;
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(result) = job.try_take_completion() {
            return Ok(result);
        }
        drop(
            wake.1
                .wait_while(wake.0.lock().map_err(|_| "Wake poisoned")?, |value| *value == revision)
                .map_err(|_| "Wake poisoned")?,
        );
    }
}

fn exchange(path: &str, content_type: &str, body: &[u8]) -> Exchange {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    Exchange {
        path: path.into(),
        range: None,
        response,
        await_client_close: true,
        cancel_client: None,
    }
}

fn qualified_peer(port: u16, context: &JobContext) -> io::Result<BackendPeer> {
    let mut transport = Transport::default();
    let listener = AttachedListenerLease::new();
    let result = (|| -> io::Result<BackendPeer> {
        assert!(TcpOwnerWorkspace::new(context).is_err());
        let mut workspace = TcpOwnerWorkspace::for_output(context).map_err(io::Error::other)?;
        assert!(
            sugarsubstitute_supervision::backend_listener::loopback_listener_present(
                port,
                &mut workspace,
                context,
            )
            .is_err()
        );
        transport.connect_loopback(port, context).map_err(io::Error::other)?;
        transport.request_loopback("/qualify", context).map_err(io::Error::other)?;
        transport.send_loopback(context).map_err(io::Error::other)?;
        transport.receive(context).map_err(io::Error::other)?;
        let (client, server) = transport.loopback_tuple(port, context).map_err(io::Error::other)?;
        let connection = ConnectedLoopback::new(client, server).map_err(io::Error::other)?;
        let actual = listener
            .qualify_connection(connection, &mut workspace, context)
            .map_err(io::Error::other)?;
        Ok(BackendPeer {
            process_id: NonZeroU32::new(actual.process_id())
                .ok_or_else(|| io::Error::other("PID"))?,
            creation_time_100ns: NonZeroU64::new(actual.creation_time_100ns())
                .ok_or_else(|| io::Error::other("Birth"))?,
        })
    })();
    let close_http = transport.close().map_err(io::Error::other);
    let close_listener = listener.close(context).map_err(io::Error::other);
    let peer = result?;
    close_http?;
    close_listener?;
    Ok(peer)
}

fn start_server(
    client: &ExecutionClient,
    scope: &Scope,
    context: &JobContext,
    exchange: Exchange,
    id: u64,
) -> io::Result<(OwnedJob<(), io::Error>, u16)> {
    let server = Server::bind(vec![exchange], context)?;
    let port = server.port()?;
    let job = client
        .submit_async(
            scope,
            JobSpec {
                id: JobId::new(id).map_err(io::Error::other)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::IoLifecycle,
                deadline: context.deadline(),
                reservation_bytes: 1024 * 1024,
                replacement: None,
            },
            move |context| server.run(context),
        )
        .map_err(io::Error::other)?;
    Ok((job, port))
}

struct Exercise {
    servers: Vec<OwnedJob<(), io::Error>>,
    transfer: io::Result<()>,
}

struct Fixture {
    servers: Vec<OwnedJob<(), io::Error>>,
    peer: BackendPeer,
    valid_port: u16,
    rejected_port: u16,
    jpeg_port: u16,
    mislabeled_port: u16,
    digest_port: u16,
    artifact_port: u16,
    cache: RuntimeCache,
}

fn setup(
    context: JobContext,
    client: ExecutionClient,
    scope: Arc<Scope>,
    anchor: PathBuf,
) -> io::Result<Fixture> {
    let target_path = "/view?filename=image.png&subfolder=&type=output";
    let (identity_server, identity_port) = start_server(
        &client,
        &scope,
        &context,
        exchange("/qualify", "application/octet-stream", b"q"),
        2,
    )?;
    let (valid_server, valid_port) =
        start_server(&client, &scope, &context, exchange(target_path, "image/png", PNG), 3)?;
    let (rejected_server, rejected_port) =
        start_server(&client, &scope, &context, exchange(target_path, "image/png", PNG), 4)?;
    let (jpeg_server, jpeg_port) =
        start_server(&client, &scope, &context, exchange(JPEG_TARGET, "image/jpeg", JPEG), 6)?;
    let (mislabeled_server, mislabeled_port) =
        start_server(&client, &scope, &context, exchange(JPEG_TARGET, "image/png", JPEG), 7)?;
    let (digest_server, digest_port) =
        start_server(&client, &scope, &context, exchange(target_path, "image/png", PNG), 9)?;
    let (artifact_server, artifact_port) = start_server(
        &client,
        &scope,
        &context,
        exchange("/view?filename=image.gif&subfolder=&type=temp", "image/gif", GIF),
        11,
    )?;
    let peer = qualified_peer(identity_port, &context)?;
    assert_eq!(peer.process_id.get(), std::process::id());
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), &context)?;
    Ok(Fixture {
        servers: vec![
            identity_server,
            valid_server,
            rejected_server,
            jpeg_server,
            mislabeled_server,
            digest_server,
            artifact_server,
        ],
        peer,
        valid_port,
        rejected_port,
        jpeg_port,
        mislabeled_port,
        digest_port,
        artifact_port,
        cache,
    })
}

/// Bind one admitted output filename to the fixture server on `port` with the granted `peer`.
fn bound(port: u16, filename: &str, peer: BackendPeer) -> io::Result<ComfyOutputTransfer> {
    let origin = LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)
        .map_err(io::Error::other)?;
    let reference = CanvasFileReference::decode(
        format!(r#"{{"filename":"{filename}","subfolder":"","type":"output"}}"#).as_bytes(),
    )
    .map_err(io::Error::other)?;
    let target = ComfyViewTarget::new(&reference).map_err(io::Error::other)?;
    ComfyOutputTransfer::new(origin, target, peer, 1024).map_err(io::Error::other)
}

/// Fetch through `transfer` and return the retained bytes after its handles settle.
fn fetched(
    transfer: &ComfyOutputTransfer,
    cache: &RuntimeCache,
    context: &JobContext,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    transfer
        .fetch(cache.clone(), utc_seconds()?, context)
        .map_err(io::Error::other)?
        .read_to_end(&mut bytes)?;
    assert!(transfer.settled());
    Ok(bytes)
}

/// Require `transfer` to fail at `stage` and still retire its handles.
fn rejected_at(
    transfer: &ComfyOutputTransfer,
    cache: &RuntimeCache,
    context: &JobContext,
    stage: &str,
) -> io::Result<()> {
    let cause = transfer
        .fetch(cache.clone(), utc_seconds()?, context)
        .err()
        .ok_or_else(|| io::Error::other(format!("Transfer passed {stage}")))?;
    assert_eq!(cause.stage, stage);
    assert!(transfer.settled());
    Ok(())
}

fn exercise(context: JobContext, fixture: Fixture) -> Exercise {
    let Fixture {
        servers,
        peer,
        valid_port,
        rejected_port,
        jpeg_port,
        mislabeled_port,
        digest_port,
        artifact_port,
        cache,
    } = fixture;
    let result = (|| -> io::Result<()> {
        use sha2::{Digest, Sha256};
        let expected =
            OutputArtifactIdentity::new(&format!("{:x}", Sha256::digest(PNG)), PNG.len() as u64)
                .map_err(io::Error::other)?;
        let raw = bound(valid_port, "image.png", peer)?;
        let verified =
            ComfyOutputTransfer::verified(raw.origin, raw.target, raw.peer, raw.maximum, expected)
                .map_err(io::Error::other)?;
        assert_eq!(fetched(&verified, &cache, &context)?, PNG);
        let raw = bound(digest_port, "image.png", peer)?;
        let mismatch = OutputArtifactIdentity::new(&"0".repeat(64), PNG.len() as u64)
            .map_err(io::Error::other)?;
        let rejected =
            ComfyOutputTransfer::verified(raw.origin, raw.target, raw.peer, raw.maximum, mismatch)
                .map_err(io::Error::other)?;
        rejected_at(&rejected, &cache, &context, "output_body")?;
        let origin =
            LoopbackOrigin::parse(&format!("http://127.0.0.1:{artifact_port}"), artifact_port)
                .map_err(io::Error::other)?;
        let reference = CanvasFileReference::decode(
            br#"{"filename":"image.gif","subfolder":"","type":"temp"}"#,
        )
        .map_err(io::Error::other)?;
        let target = ComfyViewTarget::artifact(&reference).map_err(io::Error::other)?;
        let identity =
            OutputArtifactIdentity::new(&format!("{:x}", Sha256::digest(GIF)), GIF.len() as u64)
                .map_err(io::Error::other)?;
        let generic = ComfyOutputTransfer::verified(origin, target, peer, 1024, identity)
            .map_err(io::Error::other)?;
        assert_eq!(fetched(&generic, &cache, &context)?, GIF);
        super::retained::qualify(&cache, &context, PNG)?;
        let changed = BackendPeer {
            creation_time_100ns: NonZeroU64::new(
                peer.creation_time_100ns
                    .get()
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("Birth"))?,
            )
            .ok_or_else(|| io::Error::other("Birth"))?,
            ..peer
        };
        let changed = bound(rejected_port, "image.png", changed)?;
        rejected_at(&changed, &cache, &context, "output_peer_changed")?;
        assert_eq!(fetched(&bound(jpeg_port, "image.jpg", peer)?, &cache, &context)?, JPEG);
        let mislabeled = bound(mislabeled_port, "image.jpg", peer)?;
        rejected_at(&mislabeled, &cache, &context, "output_headers")?;

        let poisoned = bound(rejected_port, "image.png", peer)?;
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poisoned.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            std::panic::resume_unwind(Box::new("test poison"));
        }));
        assert!(failure.is_err());
        assert!(!poisoned.settled());
        poisoned.retire(&context).map_err(io::Error::other)?;
        assert!(poisoned.settled());
        Ok(())
    })();
    Exercise { servers, transfer: result }
}

#[test]
fn actual_http_transfer_requires_the_granted_process_birth_and_retires_its_handles()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(ManualClock::new(), wake.clone())?;
    let client = owner.client();
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    let worker_client = client.clone();
    let worker_scope = scope.clone();
    let anchor = directory.path().to_path_buf();
    let mut preparation = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(client.now()?.checked_add(Duration::from_secs(20))?),
            // Seven fixture servers claim 5 MiB each beside the TCP owner workspace.
            reservation_bytes: 96 * 1024 * 1024,
            replacement: None,
        },
        move |context| setup(context, worker_client, worker_scope, anchor),
    )?;
    let fixture = match take(&client, &wake, &mut preparation)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        other => return Err(format!("Fixture preparation stopped: {:?}", other.kind()).into()),
    };
    let mut fixture = Some(fixture);
    let mut task = client.submit_blocking_owned_input(
        &scope,
        JobSpec {
            id: JobId::new(12)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(client.now()?.checked_add(Duration::from_secs(15))?),
            reservation_bytes: OUTPUT_TRANSFER_BYTES,
            replacement: None,
        },
        &mut fixture,
        |fixture, context| Ok::<_, io::Error>(exercise(context, fixture)),
    )?;
    let Exercise { mut servers, transfer } = match take(&client, &wake, &mut task)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        other => return Err(format!("Output transfer stopped: {:?}", other.kind()).into()),
    };
    // WinHTTP may retain an idle socket in its connection pool after request handles close.
    for server in &servers {
        server.cancel();
    }
    let mut fixture = Vec::new();
    for (index, server) in servers.iter_mut().enumerate() {
        fixture.push(match take(&client, &wake, server)?.outcome {
            JobOutcome::Success(()) | JobOutcome::Cancelled => Ok(()),
            JobOutcome::Failure(cause) => Err(format!("HTTP fixture {index} failed: {cause}")),
            other => Err(format!("HTTP fixture stopped: {:?}", other.kind())),
        });
    }
    if let Err(cause) = transfer {
        return Err(format!("Transfer failed: {cause}; fixtures: {fixture:?}").into());
    }
    for result in fixture {
        result.map_err(io::Error::other)?;
    }
    drop(servers);
    drop(task);
    drop(preparation);
    scope.cancel();
    drop(scope);
    let cutoff: Moment = client.now()?.checked_add(Duration::from_secs(10))?;
    owner.request_shutdown(cutoff);
    loop {
        let revision = *wake.0.lock().map_err(|_| "Wake poisoned")?;
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if owner.poll_shutdown()?.settled {
            break;
        }
        // The execution owner publishes a wake after its last worker returns from shutdown.
        drop(
            wake.1
                .wait_while(wake.0.lock().map_err(|_| "Wake poisoned")?, |value| *value == revision)
                .map_err(|_| "Wake poisoned")?,
        );
    }
    owner.finish_shutdown()?;
    Ok(())
}
