// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact metadata stream handles and extents across separately admitted finite jobs.
use super::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::setup_storage::{MetadataRead, MetadataWrite};

#[derive(Default)]
/// One original file transfer, carried between finite execution jobs without recreating handles.
pub(in crate::setup::worker) struct Session {
    transfer: Option<Transfer>,
}
enum Transfer {
    Read(MetadataRead),
    Write { writer: MetadataWrite, offset: u64 },
}
impl Session {
    /// Apply exactly one validated operation; errors retain partial files for explicit abort or close.
    pub(super) fn perform(
        &mut self,
        access: &ProvisioningMetadata,
        request: Request,
        context: &JobContext,
    ) -> io::Result<Value> {
        request.validate()?;
        match request {
            Request::Abort => {
                self.transfer = None;
                Ok(json!({}))
            }
            Request::Read { name } if self.transfer.is_none() => {
                self.transfer = Some(Transfer::Read(access.read(name, context)?));
                self.next(context)
            }
            Request::Next => self.next(context),
            Request::Begin { name, length } if self.transfer.is_none() => {
                self.transfer = Some(Transfer::Write {
                    writer: access.write(name, length, context)?,
                    offset: 0,
                });
                Ok(json!({}))
            }
            Request::Chunk { offset, hex } => {
                let Some(Transfer::Write { writer, offset: actual }) = &mut self.transfer else {
                    return Err(invalid());
                };
                if offset != *actual {
                    return Err(invalid());
                }
                let bytes: Vec<u8> = hex
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|pair| digit(pair[0]) * 16 + digit(pair[1]))
                    .collect();
                writer.append(&bytes, context)?;
                *actual += bytes.len() as u64;
                Ok(json!({}))
            }
            Request::Commit => {
                let Some(Transfer::Write { writer, .. }) = self.transfer.take() else {
                    return Err(invalid());
                };
                writer.commit(context)?;
                Ok(json!({}))
            }
            _ => Err(invalid()),
        }
    }
    fn next(&mut self, context: &JobContext) -> io::Result<Value> {
        let Some(Transfer::Read(reader)) = &mut self.transfer else {
            return Err(invalid());
        };
        let bytes = reader.next(context)?;
        let done = reader.done();
        let hex: String = bytes
            .as_slice()
            .iter()
            .flat_map(|byte| {
                const DIGITS: &[u8] = b"0123456789abcdef";
                [DIGITS[(byte >> 4) as usize] as char, DIGITS[(byte & 15) as usize] as char]
            })
            .collect();
        if done {
            self.transfer = None;
        }
        Ok(json!({"hex":hex,"done":done}))
    }
}
fn digit(byte: u8) -> u8 {
    if byte <= b'9' { byte - b'0' } else { byte - b'a' + 10 }
}
