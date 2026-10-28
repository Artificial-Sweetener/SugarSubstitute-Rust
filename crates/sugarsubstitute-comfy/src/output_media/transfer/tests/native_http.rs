// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify a real connected loopback peer and transfer under the shared execution owner.

use super::*;
use std::{
    io::{self, Read},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    CompletionWake, ExecutionError, ExecutionPort, JobClass, JobCompletion, JobId, JobOutcome,
    JobSpec, Moment, Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock,
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
    let until = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Deadline overflow")?;
    loop {
        let revision = *wake.0.lock().map_err(|_| "Wake poisoned")?;
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(result) = job.try_take_completion() {
            return Ok(result);
        }
        let remaining = until.checked_duration_since(Instant::now()).ok_or("Job did not settle")?;
        let (guard, timed) = wake
            .1
            .wait_timeout_while(wake.0.lock().map_err(|_| "Wake poisoned")?, remaining, |value| {
                *value == revision
            })
            .map_err(|_| "Wake poisoned")?;
        drop(guard);
        if timed.timed_out() {
            return Err("Job did not settle".into());
        }
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
    let peer = qualified_peer(identity_port, &context)?;
    assert_eq!(peer.process_id.get(), std::process::id());
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), &context)?;
    Ok(Fixture {
        servers: vec![identity_server, valid_server, rejected_server],
        peer,
        valid_port,
        rejected_port,
        cache,
    })
}

fn exercise(context: JobContext, fixture: Fixture) -> Exercise {
    let Fixture { servers, peer, valid_port, rejected_port, cache } = fixture;
    let result = (|| -> io::Result<()> {
        let origin = LoopbackOrigin::parse(&format!("http://127.0.0.1:{valid_port}"), valid_port)
            .map_err(io::Error::other)?;
        let reference = CanvasFileReference::decode(
            br#"{"filename":"image.png","subfolder":"","type":"output"}"#,
        )
        .map_err(io::Error::other)?;
        let target = ComfyViewTarget::new(&reference).map_err(io::Error::other)?;
        let valid = ComfyOutputTransfer::new(origin, target.clone(), peer, 1024)
            .map_err(io::Error::other)?;
        let mut reader =
            valid.fetch(cache.clone(), utc_seconds()?, &context).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        assert_eq!(bytes, PNG);
        drop(reader);
        assert!(valid.settled());

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
        let origin =
            LoopbackOrigin::parse(&format!("http://127.0.0.1:{rejected_port}"), rejected_port)
                .map_err(io::Error::other)?;
        let rejected = ComfyOutputTransfer::new(origin, target.clone(), changed, 1024)
            .map_err(io::Error::other)?;
        let cause = rejected
            .fetch(cache, utc_seconds()?, &context)
            .err()
            .ok_or_else(|| io::Error::other("Changed peer was accepted"))?;
        assert_eq!(cause.stage, "output_peer_changed");
        assert!(rejected.settled());

        let poisoned =
            ComfyOutputTransfer::new(origin, target, peer, 1024).map_err(io::Error::other)?;
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
    let mut owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
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
            reservation_bytes: 32 * 1024 * 1024,
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
            id: JobId::new(5)?,
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
    for server in &mut servers {
        fixture.push(match take(&client, &wake, server)?.outcome {
            JobOutcome::Success(()) | JobOutcome::Cancelled => Ok(()),
            JobOutcome::Failure(cause) => Err(format!("HTTP fixture failed: {cause}")),
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
    let until = Instant::now().checked_add(Duration::from_secs(15)).ok_or("Deadline overflow")?;
    loop {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        let progress = owner.poll_shutdown()?;
        if progress.settled {
            break;
        }
        if Instant::now() >= until {
            return Err(format!("Owner did not settle: {progress:?}").into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}
