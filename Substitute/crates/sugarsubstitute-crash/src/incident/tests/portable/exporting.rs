// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export success requires complete original bytes and unchanged current copied-source custody.
use super::*;
use std::{io, rc::Rc};

#[test]
fn original_attachment_streams_exactly_and_reopens_without_acknowledgement() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let expected = b"the original redacted diagnostic attachment";
        let key = source.retained(expected)?;
        let original = source.original_bytes();
        let mut source = source.copied();
        for _ in 0..2 {
            let mut history = project_portable_committed(
                &mut source,
                NamespaceId::new([1; 32]),
                now()?,
                &context,
            )?;
            let mut output = Vec::new();
            let exported =
                history.export_attachment(key, AttachmentKind::Fault, &mut output, &context)?;
            assert_eq!(output, expected);
            assert_eq!(exported.bytes(), expected.len() as u64);
            assert_eq!(exported.sha256(), crate::incident::codec_ledger::digest(expected));
            assert!(
                history
                    .summaries()
                    .next()
                    .ok_or("Summary missing")?
                    .summary()
                    .facts()
                    .retention()
                    .acknowledged()
                    .is_none()
            );
            drop(history);
        }
        assert!(source.unchanged(&original));
        Ok(())
    })
}

#[test]
fn changed_pinned_generation_and_sink_error_never_return_export_success() -> TestResult {
    admitted(|context| {
        let mut source = Source::new()?;
        let key = source.retained(b"original attachment")?;
        let generation = Rc::clone(&source.generation);
        let mut history =
            project_portable_committed(&mut source, NamespaceId::new([1; 32]), now()?, &context)?;
        let renewed = context.bounded_until(context.now());
        let mut rejected_output = Vec::new();
        let cause = history
            .export_attachment(key, AttachmentKind::Fault, &mut rejected_output, &renewed)
            .err()
            .ok_or("Expired context renewed export")?;
        assert_eq!(cause.code(), PortableIncidentFailureCode::Deadline);
        assert!(rejected_output.is_empty());
        drop(cause);
        generation.set(2);
        let mut output = Vec::new();
        let cause = history
            .export_attachment(key, AttachmentKind::Fault, &mut output, &context)
            .err()
            .ok_or("Changed source accepted")?;
        assert_eq!(cause.code(), PortableIncidentFailureCode::ChangedSource);
        assert!(output.is_empty());
        drop(cause);
        generation.set(1);
        struct FailedSink;
        impl io::Write for FailedSink {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("actual export sink failed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let cause = history
            .export_attachment(key, AttachmentKind::Fault, &mut FailedSink, &context)
            .err()
            .ok_or("Failed sink accepted")?;
        assert_eq!(cause.code(), PortableIncidentFailureCode::Source);
        assert!(std::error::Error::source(&cause).is_some());
        Ok(())
    })
}

#[test]
fn corrupt_payload_and_unassociated_payload_refuse_before_any_export() -> TestResult {
    admitted(|context| {
        for associated in [true, false] {
            let mut source = Source::new()?;
            let key = source.retained(b"original attachment")?;
            if associated {
                source
                    .rows
                    .iter_mut()
                    .find(|row| row.class == DiagnosticEntryClass::Payload)
                    .ok_or("Payload missing")?
                    .bytes[0] ^= 1;
            } else {
                source.add(
                    DiagnosticEntryClass::Payload,
                    Some(DiagnosticObject::Payload(
                        key,
                        IncidentRevision::new(9)?,
                        AttachmentKind::Fault,
                    )),
                    b"unassociated".to_vec(),
                )?;
            }
            let failure = match project_portable_committed(
                &mut source,
                NamespaceId::new([1; 32]),
                now()?,
                &context,
            ) {
                Ok(_) => return Err("Corrupt/unassociated payload accepted".into()),
                Err(cause) => cause,
            };
            assert_eq!(
                failure.code(),
                if associated {
                    PortableIncidentFailureCode::WrongDigest
                } else {
                    PortableIncidentFailureCode::Uncommitted
                }
            );
        }
        Ok(())
    })
}
