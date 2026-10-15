// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode only closed command-specific fields without allocation.
use super::super::{reader::Reader, *};
use super::fields::*;
use DiagnosticWireError as E;
use sugarsubstitute_contracts::incident::*;
pub(in crate::diagnostic_client) fn read<'a>(
    code: u8,
    r: &mut Reader<'_>,
    payload: &'a [u8],
    context: DiagnosticContext,
) -> Result<DiagnosticCommand<'a>, E> {
    use DiagnosticCommand as C;
    let c = match code {
        31 => C::Result {
            kind: DiagnosticRequestKind::from_code(r.u8()?)?,
            request_sha256: r.array()?,
            reply: read_reply(r)?,
        },
        1 => C::OpenProtectedRoot,
        2 => C::Recover,
        3 => C::Seed(payload),
        4 => {
            let expected = match r.u8()? {
                0 => DiagnosticExpectedSummary::Absent,
                1 => {
                    DiagnosticExpectedSummary::Exact { revision: revision(r)?, sha256: r.array()? }
                }
                _ => return Err(E::Schema),
            };
            let demand = DiagnosticPublicationDemand::new(r.u64()?, r.u32()?, r.u32()?, r.u64()?)?;
            C::Publish(DiagnosticPublication::Begin { expected, demand, seed: payload })
        }
        5 => C::Publish(DiagnosticPublication::Chunk(read_chunk(r, payload)?)),
        6 => C::Publish(DiagnosticPublication::Finish { bytes: r.u64()?, sha256: r.array()? }),
        7 => C::Load,
        8 => C::RetainExport(read_choice(r, payload)?),
        9 => C::ExportChunk(read_chunk(r, payload)?),
        10 => C::FinishExport(r.array()?),
        11 => C::Acknowledge { session: session(r)?, restart: r.boolean()? },
        12 => {
            let session = session(r)?;
            let attempt = match r.u8()? {
                0 => ReportAttempt::Presented,
                c => ReportAttempt::Failed(failure(c)?),
            };
            C::RecordAttempt { session, attempt }
        }
        13 => C::RetainDump(DumpId::new(r.array()?).map_err(|_| E::Identity)?),
        14 => C::Symbolize {
            dump: DumpId::new(r.array()?).map_err(|_| E::Identity)?,
            release: r.array()?,
        },
        15 => C::DiscardCapture(DumpId::new(r.array()?).map_err(|_| E::Identity)?),
        16 => C::Retire,
        17 => C::BeginView {
            session: session(r)?,
            restart: restart(r)?,
            total: r.u64()?,
            sha256: r.array()?,
        },
        18 => C::ViewChunk { session: session(r)?, chunk: read_chunk(r, payload)? },
        19 => C::EndView { session: session(r)?, sha256: r.array()? },
        20 => C::ReplaceView {
            session: session(r)?,
            restart: restart(r)?,
            old: revision(r)?,
            total: r.u64()?,
            sha256: r.array()?,
        },
        21 => C::Presented(session(r)?),
        22 => C::ViewReplaced(session(r)?),
        23 => C::Action { session: session(r)?, action: action(r.u8()?)? },
        24 => C::SaveSelectionReady { session: session(r)?, selection: read_choice(r, payload)? },
        25 => C::SaveSelectionCancelled(session(r)?),
        26 => C::NativeRetired(session(r)?),
        27 => C::ReporterFailed { session: session(r)?, failure: failure(r.u8()?)? },
        28 => {
            let session = session(r)?;
            let subject = context.incident.ok_or(E::Shape)?;
            let at = IncidentUtc::from_unix_seconds(r.i64()?).map_err(|_| E::Time)?;
            C::CloseAfterDurableAck {
                session,
                acknowledgement: DurableAcknowledgement::new(subject.key, subject.revision, at),
            }
        }
        29 => C::CopyResult { session: session(r)?, result: read_result(r)? },
        30 => C::ExportSelectionResult { session: session(r)?, result: read_result(r)? },
        32 => C::RestartUnavailable { session: session(r)? },
        _ => return Err(E::Schema),
    };
    if !matches!(code, 3 | 4 | 5 | 8 | 9 | 18 | 24) && !payload.is_empty() {
        return Err(E::Shape);
    }
    Ok(c)
}

/// Accept only the two closed presentation values; no readiness privilege is decoded.
fn restart(r: &mut Reader<'_>) -> Result<ReportRestartAvailability, E> {
    Ok(if r.boolean()? {
        ReportRestartAvailability::Available
    } else {
        ReportRestartAvailability::Unavailable
    })
}
