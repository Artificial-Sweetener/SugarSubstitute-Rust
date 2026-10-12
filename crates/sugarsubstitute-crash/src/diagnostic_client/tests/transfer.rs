// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Complete view transfer remains separate from native presentation/buffer authority.
use super::{super::*, support::*};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::incident::*;
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn chunk<'a>(bytes: &'a [u8], offset: u64, total: u64, sha256: [u8; 32]) -> DiagnosticChunk<'a> {
    DiagnosticChunk { kind: DiagnosticTransferKind::View, total, offset, sha256, bytes }
}
#[test]
fn partial_view_cannot_complete_and_split_unicode_completes_once() -> TestResult {
    let b = view_binding()?;
    let bytes = "A日🍭".as_bytes();
    let hash = digest(bytes);
    let mut t = DiagnosticTransfer::new(b, DiagnosticTransferKind::View, bytes.len() as u64, hash)?;
    t.accept(b, chunk(&bytes[..3], 0, bytes.len() as u64, hash))?;
    assert_eq!(t.finish(b, hash).err(), Some(DiagnosticWireError::Incomplete));
    t.accept(b, chunk(&bytes[3..], 3, bytes.len() as u64, hash))?;
    let done = t.finish(b, hash)?;
    assert_eq!(done.bytes(), bytes.len() as u64);
    assert_eq!(done.binding(), b);
    assert_eq!(done.sha256(), hash);
    assert_eq!(t.finish(b, hash).err(), Some(DiagnosticWireError::Stale));
    Ok(())
}
#[test]
fn changed_binding_offset_kind_total_digest_preserve_original_progress() -> TestResult {
    let b = view_binding()?;
    let hash = digest(b"ab");
    let mut t = DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 2, hash)?;
    t.accept(b, chunk(b"a", 0, 2, hash))?;
    let mut changed_frame = reporter(
        1,
        DiagnosticCommand::BeginView { session: ReportSessionId::new(8)?, total: 2, sha256: hash },
        true,
    )?;
    changed_frame.context.incident.as_mut().ok_or("Missing incident")?.revision =
        subject()?.revision.next()?;
    let changed = DiagnosticTransferBinding::from_frame(&changed_frame)?;
    assert_eq!(t.accept(changed, chunk(b"b", 1, 2, hash)), Err(DiagnosticWireError::Chunk));
    for case in 0..4 {
        let mut c = chunk(b"b", 1, 2, hash);
        match case {
            0 => c.offset = 0,
            1 => c.kind = DiagnosticTransferKind::Summary,
            2 => c.total = 3,
            _ => c.sha256 = [1; 32],
        };
        assert_eq!(t.accept(b, c), Err(DiagnosticWireError::Chunk));
        assert_eq!(t.accepted_bytes(), 1)
    }
    t.accept(b, chunk(b"b", 1, 2, hash))?;
    t.finish(b, hash)?;
    Ok(())
}
#[test]
fn invalid_utf8_chunk_is_atomic_and_can_be_corrected_at_same_offset() -> TestResult {
    let b = view_binding()?;
    let hash = digest(b"a");
    let mut t = DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 1, hash)?;
    assert_eq!(t.accept(b, chunk(&[0xff], 0, 1, hash)), Err(DiagnosticWireError::Integrity));
    assert_eq!(t.accepted_bytes(), 0);
    t.accept(b, chunk(b"a", 0, 1, hash))?;
    t.finish(b, hash)?;
    Ok(())
}
#[test]
fn truncated_overlong_surrogate_and_out_of_range_utf8_never_complete() -> TestResult {
    let b = view_binding()?;
    for bytes in [&[0xe0, 0x80, 0x80][..], &[0xed, 0xa0, 0x80], &[0xf4, 0x90, 0x80, 0x80]] {
        let hash = digest(bytes);
        let mut t =
            DiagnosticTransfer::new(b, DiagnosticTransferKind::View, bytes.len() as u64, hash)?;
        assert_eq!(
            t.accept(b, chunk(bytes, 0, bytes.len() as u64, hash)),
            Err(DiagnosticWireError::Integrity)
        );
    }
    let hash = digest(&[0xe3, 0x81]);
    let mut t = DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 2, hash)?;
    t.accept(b, chunk(&[0xe3, 0x81], 0, 2, hash))?;
    assert_eq!(t.finish(b, hash).err(), Some(DiagnosticWireError::Incomplete));
    Ok(())
}
#[test]
fn exact_complete_byte_count_does_not_replace_digest_validation() -> TestResult {
    let b = view_binding()?;
    let hash = digest(b"b");
    let mut t = DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 1, hash)?;
    t.accept(b, chunk(b"a", 0, 1, hash))?;
    assert_eq!(t.finish(b, hash).err(), Some(DiagnosticWireError::Integrity));
    Ok(())
}
#[test]
fn opaque_native_dump_is_not_mislabeled_sanitized_utf8() -> TestResult {
    let b = storage_binding()?;
    let bytes = [0xff, 0x00];
    let hash = digest(&bytes);
    let kind = DiagnosticTransferKind::Attachment(AttachmentKind::Minidump);
    let mut t = DiagnosticTransfer::new(b, kind, 2, hash)?;
    t.accept(b, DiagnosticChunk { kind, total: 2, offset: 0, sha256: hash, bytes: &bytes })?;
    assert_eq!(t.finish(b, hash)?.kind(), kind);
    Ok(())
}
#[test]
fn view_and_summary_total_bounds_are_separate_from_dump_bound() -> TestResult {
    let b = view_binding()?;
    assert_eq!(
        DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 262_145, [0; 32]).err(),
        Some(DiagnosticWireError::Capacity)
    );
    assert_eq!(
        DiagnosticTransfer::new(
            storage_binding()?,
            DiagnosticTransferKind::Summary,
            65_537,
            [0; 32]
        )
        .err(),
        Some(DiagnosticWireError::Capacity)
    );
    assert_eq!(
        DiagnosticTransfer::new(b, DiagnosticTransferKind::View, 0, [0; 32]).err(),
        Some(DiagnosticWireError::Capacity)
    );
    Ok(())
}

fn view_binding() -> Result<DiagnosticTransferBinding, Box<dyn std::error::Error>> {
    Ok(DiagnosticTransferBinding::from_frame(&reporter(
        1,
        DiagnosticCommand::BeginView {
            session: ReportSessionId::new(8)?,
            total: 2,
            sha256: [0; 32],
        },
        true,
    )?)?)
}
fn storage_binding() -> Result<DiagnosticTransferBinding, Box<dyn std::error::Error>> {
    Ok(DiagnosticTransferBinding::from_frame(&storage(
        1,
        DiagnosticCommand::Publish(DiagnosticPublication::Begin {
            expected: DiagnosticExpectedSummary::Absent,
            demand: DiagnosticPublicationDemand::new(0, 0, 1, 131_072)?,
            seed: b"{}",
        }),
    )?)?)
}
#[test]
fn storage_stream_retains_full_current_key_operation_deadline_and_utc() -> TestResult {
    let mut original = storage(
        1,
        DiagnosticCommand::Publish(DiagnosticPublication::Finish {
            bytes: 2,
            sha256: digest(b"ab"),
        }),
    )?;
    let binding = DiagnosticTransferBinding::from_frame(&original)?;
    assert_eq!(binding.session(), None);
    assert_eq!(binding.key(), original.key);
    assert_eq!(binding.context(), original.context);
    let kind = DiagnosticTransferKind::Summary;
    let mut transfer = DiagnosticTransfer::new(binding, kind, 2, digest(b"ab"))?;
    let mut c = chunk(b"a", 0, 2, digest(b"ab"));
    c.kind = kind;
    transfer.accept(binding, c)?;
    for case in 0..5 {
        let mut changed = original;
        match case {
            0 => {
                changed.key = DiagnosticKey::new(
                    owner(1)?,
                    DiagnosticRole::Storage,
                    DiagnosticSessionId::new(9)?,
                )
            }
            1 => {
                changed.context.operation =
                    Some(LogicalStoreOperationKey::new(owner(1)?, IncidentOperationId::new(9)?))
            }
            2 => changed.context.deadline = Some(DiagnosticDeadline::new(101, 10)?),
            3 => changed.context.at = Some(IncidentUtc::from_unix_seconds(21)?),
            _ => {
                changed.context.incident.as_mut().ok_or("Missing incident")?.revision =
                    subject()?.revision.next()?
            }
        }
        let wrong = DiagnosticTransferBinding::from_frame(&changed)?;
        let mut next = c;
        next.offset = 1;
        next.bytes = b"b";
        assert_eq!(transfer.accept(wrong, next), Err(DiagnosticWireError::Chunk));
        assert_eq!(transfer.accepted_bytes(), 1);
    }
    original.request = DiagnosticRequestId::new(2)?;
    assert_eq!(DiagnosticTransferBinding::from_frame(&original)?, binding);
    c.offset = 1;
    c.bytes = b"b";
    transfer.accept(binding, c)?;
    transfer.finish(binding, digest(b"ab"))?;
    Ok(())
}
#[test]
fn reporter_stream_rejects_changed_current_session_and_reporter_binding() -> TestResult {
    let original = reporter(
        1,
        DiagnosticCommand::BeginView {
            session: ReportSessionId::new(8)?,
            total: 2,
            sha256: digest(b"ab"),
        },
        true,
    )?;
    let binding = DiagnosticTransferBinding::from_frame(&original)?;
    let mut transfer =
        DiagnosticTransfer::new(binding, DiagnosticTransferKind::View, 2, digest(b"ab"))?;
    for case in 0..3 {
        let mut wrong = original;
        match case {
            0 => {
                wrong.key = DiagnosticKey::new(
                    owner(1)?,
                    DiagnosticRole::Reporter,
                    DiagnosticSessionId::new(9)?,
                )
            }
            1 => {
                wrong.command = DiagnosticCommand::BeginView {
                    session: ReportSessionId::new(9)?,
                    total: 2,
                    sha256: digest(b"ab"),
                }
            }
            _ => wrong.context.deadline = Some(DiagnosticDeadline::new(101, 10)?),
        }
        assert_eq!(
            transfer.accept(
                DiagnosticTransferBinding::from_frame(&wrong)?,
                chunk(b"a", 0, 2, digest(b"ab"))
            ),
            Err(DiagnosticWireError::Chunk)
        );
        assert_eq!(transfer.accepted_bytes(), 0);
    }
    Ok(())
}
#[test]
fn nonstream_command_and_wrong_transfer_role_cannot_create_a_stream() -> TestResult {
    assert_eq!(
        DiagnosticTransferBinding::from_frame(&storage(1, DiagnosticCommand::Load)?).err(),
        Some(DiagnosticWireError::Shape)
    );
    assert_eq!(
        DiagnosticTransfer::new(view_binding()?, DiagnosticTransferKind::Summary, 1, [0; 32]).err(),
        Some(DiagnosticWireError::Role)
    );
    assert_eq!(
        DiagnosticTransfer::new(storage_binding()?, DiagnosticTransferKind::View, 1, [0; 32]).err(),
        Some(DiagnosticWireError::Role)
    );
    Ok(())
}
