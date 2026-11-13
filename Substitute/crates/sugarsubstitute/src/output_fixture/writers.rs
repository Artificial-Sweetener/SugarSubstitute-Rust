// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every synchronous source write belongs to a finite admitted blocking operation.
use super::{Result, corpus, options::Role};
use std::io::{self, Write};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobContext, JobSpec, OwnedJob, Scope},
    transcript::OutputStream,
};
use sugarsubstitute_execution::ExecutionClient;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Written {
    pub(super) bytes: u64,
    pub(super) writes: u64,
}
pub(super) fn submit(
    client: &ExecutionClient,
    scope: &Scope,
    spec: JobSpec,
    role: Role,
    stream: OutputStream,
) -> Result<OwnedJob<Written, io::Error>> {
    Ok(client.submit_blocking(scope, spec, move |context| match stream {
        OutputStream::Stdout => write(&context, role, stream, &mut io::stdout().lock()),
        OutputStream::Stderr => write(&context, role, stream, &mut io::stderr().lock()),
    })?)
}
pub(super) fn write(
    context: &JobContext,
    role: Role,
    stream: OutputStream,
    sink: &mut impl Write,
) -> io::Result<Written> {
    let _claim = context.memory().claim(32768).map_err(io::Error::other)?;
    let mut written = Written { bytes: 0, writes: 0 };
    if role == Role::Flood {
        let bytes = vec![if stream == OutputStream::Stdout { b'O' } else { b'E' }; 32768];
        for _ in 0..corpus::FLOOD_CHUNKS {
            checkpoint(context)?;
            sink.write_all(&bytes)?;
            written.bytes += bytes.len() as u64;
            written.writes += 1;
        }
    } else {
        let parts: &[&[u8]] = match (role, stream) {
            (Role::Corpus, OutputStream::Stdout) => corpus::OUT_PARTS,
            (Role::Corpus, OutputStream::Stderr) => corpus::ERR_PARTS,
            (Role::Leaf, OutputStream::Stdout) => &[corpus::LEAF_OUT],
            (Role::Leaf, OutputStream::Stderr) => &[corpus::LEAF_ERR],
            (Role::Prelude, OutputStream::Stdout) => &[b"O"],
            (Role::Prelude, OutputStream::Stderr) => &[b"E"],
            _ => return Err(io::Error::other("Output role does not write bytes")),
        };
        for bytes in parts {
            checkpoint(context)?;
            sink.write_all(bytes)?;
            written.bytes += bytes.len() as u64;
            written.writes += 1;
        }
    }
    checkpoint(context)?;
    sink.flush()?;
    Ok(written)
}
fn checkpoint(context: &JobContext) -> io::Result<()> {
    if context.cancellation_requested() || context.deadline().is_some_and(|at| context.now() >= at)
    {
        Err(io::Error::other("Owned output write stopped at its original boundary"))
    } else {
        Ok(())
    }
}
