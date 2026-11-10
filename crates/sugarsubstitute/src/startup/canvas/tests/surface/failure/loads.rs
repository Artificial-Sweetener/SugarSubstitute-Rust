// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fetch selected outputs over a real loopback Comfy `/view` boundary that fails each way.

use super::*;
use std::io;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{
        ExecutionPort, JobClass, JobContext, JobId, JobOutcome, JobSpec, Operation, OwnedJob, Scope,
    },
    lifecycle::{LeaseId, ResourceGeneration, ResourceId},
};
use sugarsubstitute_execution::{
    MonotonicClock,
    http_fixture::{Exchange, Server},
};
use sugarsubstitute_networking::Transport;
use sugarsubstitute_supervision::{
    backend_listener::{AttachedListenerLease, ConnectedLoopback, TcpOwnerWorkspace},
    lifecycle_protocol::{BackendPeer, LeaseReference},
};

/// Encode one opaque RGB PNG of a single colour.
fn png(color: [u8; 3]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 16, 8);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&color.repeat(16 * 8))?;
    writer.finish()?;
    Ok(bytes)
}

/// One scripted `/view` exchange. A complete response keeps the server's socket, and so its
/// connection-owner row, until the client retires the transfer; otherwise the server closes
/// right after writing `response`.
fn exchange(filename: &str, response: Vec<u8>, complete: bool) -> Exchange {
    scripted(format!("/view?filename={filename}&subfolder=&type=output"), response, complete)
}

/// One scripted request for `path`.
fn scripted(path: String, response: Vec<u8>, complete: bool) -> Exchange {
    Exchange { path, range: None, response, await_client_close: complete, cancel_client: None }
}

/// A response with `content_type` and a declared length of `declared` bytes.
fn response(content_type: &str, declared: usize, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {declared}\r\n\r\n"
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// Path of the request that ends the `/view` script.
const RETIRE: &str = "/retire";

/// The outputs, in the order the test selects them, and the server's matching responses.
fn script() -> Result<Vec<Exchange>, Box<dyn Error>> {
    let red = png([255, 0, 0])?;
    let blue = png([0, 0, 255])?;
    // A genuine signature and header followed by a corrupted image stream.
    let mut corrupt = red[..33].to_vec();
    corrupt.extend_from_slice(&[0x5a; 64]);
    Ok(vec![
        exchange("red.png", response("image/png", red.len(), &red), true),
        exchange(
            "missing.png",
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
            true,
        ),
        // The connection ends partway through the declared body.
        exchange("truncated.png", response("image/png", red.len(), &red[..red.len() / 2]), false),
        exchange("mislabeled.png", response("text/html", red.len(), &red), true),
        exchange("corrupt.png", response("image/png", corrupt.len(), &corrupt), true),
        exchange("blue.png", response("image/png", blue.len(), &blue), true),
        // The test's own final request lets the server observe the last pooled socket retire.
        scripted(RETIRE.into(), response("text/plain", 0, b""), false),
    ])
}

/// Qualify this process as the listening peer through the production listener owner.
fn qualified_peer(port: u16, context: &JobContext) -> io::Result<BackendPeer> {
    let mut transport = Transport::default();
    let listener = AttachedListenerLease::new();
    let result = (|| -> io::Result<BackendPeer> {
        let mut workspace = TcpOwnerWorkspace::for_output(context).map_err(io::Error::other)?;
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
            process_id: std::num::NonZeroU32::new(actual.process_id())
                .ok_or_else(|| io::Error::other("PID"))?,
            creation_time_100ns: std::num::NonZeroU64::new(actual.creation_time_100ns())
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

/// Bind and start one scripted server as an owned asynchronous job of the server executor.
fn serve(
    client: &ExecutionClient,
    scope: &Scope,
    context: &JobContext,
    exchanges: Vec<Exchange>,
    id: u64,
) -> io::Result<(OwnedJob<(), io::Error>, u16)> {
    let server = Server::bind(exchanges, context)?;
    let port = server.port()?;
    let spec = JobSpec {
        id: JobId::new(id).map_err(io::Error::other)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::IoLifecycle,
        deadline: context.deadline(),
        reservation_bytes: 1024 * 1024,
        replacement: None,
    };
    let job = client
        .submit_async(scope, spec, move |context| server.run(context))
        .map_err(io::Error::other)?;
    Ok((job, port))
}

/// Loopback servers on their own real-time executor, so socket polling never needs the
/// canvas's frozen clock.
struct Servers {
    owner: ExecutionOwner,
    client: ExecutionClient,
    wake: Arc<Wake>,
    /// The qualification server, then the `/view` server.
    jobs: [OwnedJob<(), io::Error>; 2],
    port: u16,
    scope: Arc<Scope>,
}

/// A finite ordinary infrastructure job on the server executor.
fn ordinary(id: u64, client: &ExecutionClient) -> Result<JobSpec, Box<dyn Error>> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(client.now()?.checked_add(Duration::from_secs(60))?),
        reservation_bytes: 48 * 1024 * 1024,
        replacement: None,
    })
}

impl Servers {
    /// Start the identity and `/view` servers and return the backend grant for `identity`.
    fn start(
        identity: MainIdentity,
    ) -> Result<(Self, crate::startup::CanvasBackend), Box<dyn Error>> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
        let client = owner.client();
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let (worker_client, worker_scope, outputs) = (client.clone(), scope.clone(), script()?);
        let mut setup = client.submit_blocking(&scope, ordinary(1, &client)?, move |context| {
            let qualify = vec![scripted(
                "/qualify".into(),
                response("application/octet-stream", 1, b"q"),
                true,
            )];
            let (identity_job, identity_port) =
                serve(&worker_client, &worker_scope, &context, qualify, 2)?;
            let (view_job, view_port) = serve(&worker_client, &worker_scope, &context, outputs, 3)?;
            let peer = qualified_peer(identity_port, &context)?;
            Ok::<_, io::Error>(([identity_job, view_job], view_port, peer))
        })?;
        let (jobs, port, peer) = match wake.completion(&client, &mut setup)?.outcome {
            JobOutcome::Success(value) => value,
            JobOutcome::Failure(cause) => return Err(cause.into()),
            other => return Err(format!("Server setup stopped: {:?}", other.kind()).into()),
        };
        let backend = crate::startup::CanvasBackend {
            identity,
            lease: LeaseReference {
                resource: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
                lease: LeaseId::new(1)?,
            },
            origin: LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)?,
            peer,
        };
        Ok((Self { owner, client, wake, jobs, port, scope }, backend))
    }

    /// Require the `/view` server to have served exactly its script, then join the executor.
    fn finish(mut self) -> Result<(), Box<dyn Error>> {
        let port = self.port;
        let mut retire = self.client.submit_blocking(&self.scope, ordinary(4, &self.client)?, {
            move |context| {
                let mut transport = Transport::default();
                let result = (|| {
                    transport.connect_loopback(port, &context)?;
                    transport.request_loopback(RETIRE, &context)?;
                    transport.send_loopback(&context)?;
                    transport.receive(&context)
                })()
                .map_err(io::Error::other);
                transport.close().map_err(io::Error::other)?;
                result
            }
        })?;
        if let JobOutcome::Failure(cause) = self.wake.completion(&self.client, &mut retire)?.outcome
        {
            return Err(cause.into());
        }
        // The qualification client may pool its socket; nothing else will connect to it.
        self.jobs[0].cancel();
        for (index, job) in self.jobs.iter_mut().enumerate() {
            match self.wake.completion(&self.client, job)?.outcome {
                JobOutcome::Success(()) => {}
                JobOutcome::Cancelled if index == 0 => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                other => return Err(format!("Loopback server stopped: {:?}", other.kind()).into()),
            }
        }
        drop((retire, self.jobs, self.scope));
        self.wake.finish_shutdown(
            &mut self.owner,
            &self.client,
            self.client.now()?.checked_add(Duration::from_secs(10))?,
        )?;
        Ok(())
    }
}

#[test]
fn missing_truncated_mislabeled_and_corrupt_outputs_fail_exactly_and_the_next_selection_recovers()
-> Result<(), Box<dyn Error>> {
    let (servers, backend) = Servers::start(identity()?)?;
    let canvas = hosted(Host::comfy(backend)?, |host| {
        host.send(1, 3, "surface.select", select("red.png"))?;
        host.expect(3, "applied")?;
        host.send(1, 4, "surface.commit", json!({}))?;
        host.expect(4, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Red));
        let committed = host.committed().len();
        for (id, filename) in
            (5..).zip(["missing.png", "truncated.png", "mislabeled.png", "corrupt.png"])
        {
            host.send(1, id, "surface.select", select(filename))?;
            answers(&host.expect(id, "error")?, Address::ORIGINAL, id);
            host.quiesce()?;
            assert_eq!(host.committed().len(), committed, "{filename}");
            assert_eq!(host.shown(), Some(Shade::Red), "{filename}");
            idle(host)?;
        }
        host.send(1, 9, "surface.select", select("blue.png"))?;
        host.expect(9, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        idle(host)
    });
    let served = servers.finish();
    canvas.and(served)
}
