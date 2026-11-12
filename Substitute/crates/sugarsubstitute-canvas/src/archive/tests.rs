// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact archive round trips and rejection of every malformed or unsupported encoding.
use super::*;
use crate::{DocumentEdit, MaskAppearance, tests::harness};
use flate2::{Compression, write::DeflateEncoder};
use sha2::{Digest, Sha256};
use std::io::Write;
use sugarsubstitute_image_ops::{AssetId, FULL_COVERAGE, Path, Pixel, Region, VectorRectangle};

const MEMORY: u64 = 64 * 1024 * 1024;

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), ArchiveError> + Send + 'static,
) -> harness::TestResult {
    harness::run(MEMORY, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn canvas(cause: CanvasError) -> ArchiveError {
    match cause {
        CanvasError::Image(cause) => ArchiveError::Image(cause),
        CanvasError::Invalid(reason) => ArchiveError::Invalid(reason),
    }
}

/// A document holding every layer kind, sparse and stored coverage, and a selection.
fn rich_document(context: &JobContext) -> Result<Document, ArchiveError> {
    let memory = context.memory();
    let extent = Extent::new(300, 200)?;
    let mut document = Document::new(7, extent, memory).map_err(canvas)?;
    let mut mask = Path::rectangle([10.5, 20.25, 140.0, 90.0])?.rasterize(extent, context)?;
    mask.update(Region::new(0, 128, 128, 72)?, memory, |_, _, _| FULL_COVERAGE)?;
    let mut selection = Coverage::new(extent, 0, memory)?;
    selection.update(Region::new(260, 10, 40, 30)?, memory, |x, y, _| (x * 97 + y) as u16)?;
    let mut layer_mask = Coverage::new(extent, FULL_COVERAGE, memory)?;
    layer_mask.update(Region::new(5, 5, 10, 10)?, memory, |x, _, _| x as u16)?;
    let layers = [
        Layer::new(
            LayerId::new(11).map_err(canvas)?,
            LayerContent::Raster { asset: AssetId([3; 32]), extent },
        )
        .and_then(|layer| layer.with_name("Base"))
        .map_err(canvas)?
        .with_locked(true),
        Layer::new(
            LayerId::new(12).map_err(canvas)?,
            LayerContent::PlacedRaster {
                asset: AssetId([4; 32]),
                extent: Extent::new(64, 48)?,
                bounds: [-3.5, 7.25, 120.0, 90.5],
            },
        )
        .and_then(|layer| layer.with_opacity(0.375))
        .map_err(canvas)?
        .with_visibility(false)
        .with_mask(Some(layer_mask)),
        Layer::new(
            LayerId::new(13).map_err(canvas)?,
            LayerContent::VectorRectangle(VectorRectangle::new(
                [-10.0, 5.0, 40.5, 60.0],
                Pixel::linear(0.25, 0.5, 1.5, 0.75)?,
            )?),
        )
        .map_err(canvas)?,
        Layer::new(LayerId::new(14).map_err(canvas)?, LayerContent::Exposure { stops: -0.0 })
            .map_err(canvas)?,
        Layer::new(
            LayerId::new(15).map_err(canvas)?,
            LayerContent::Coverage {
                coverage: mask,
                appearance: MaskAppearance::new([255, 32, 7], 0.5).map_err(canvas)?,
            },
        )
        .and_then(|layer| layer.with_name("Mask ✓"))
        .map_err(canvas)?,
    ];
    for (index, layer) in layers.into_iter().enumerate() {
        document.apply(DocumentEdit::Insert { index, layer }).map_err(canvas)?;
    }
    document.apply(DocumentEdit::Select(Some(selection))).map_err(canvas)?;
    Ok(document)
}

fn source(layer: &Layer) -> Option<SourceReference> {
    SourceReference::new(format!("input/{}.png", layer.id().get()).as_bytes()).ok()
}

fn rich_archive(context: &JobContext) -> Result<InputDocumentArchive, ArchiveError> {
    let rich = rich_document(context)?;
    let memory = context.memory();
    let extent = Extent::new(129, 1)?;
    let mut selected = Document::new(8, extent, memory).map_err(canvas)?;
    let all = Coverage::new(extent, FULL_COVERAGE, memory)?;
    selected.apply(DocumentEdit::Select(Some(all))).map_err(canvas)?;
    let empty = Document::new(9, Extent::new(1, 1)?, memory).map_err(canvas)?;
    InputDocumentArchive::new(
        vec![
            ArchivedDocument::capture("workflow-b/node-2", &rich.snapshot(), source)?,
            ArchivedDocument::capture("workflow-a/node-9", &selected.snapshot(), source)?,
            ArchivedDocument::capture("empty", &empty.snapshot(), source)?,
        ],
        b"{\"active\":\"workflow-b/node-2\"}",
    )
}

/// Seal a hand-written payload with a valid header and digest.
fn seal(version: u32, payload: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    let _ = encoder.write_all(payload);
    let body = encoder.finish().unwrap_or_default();
    let mut bytes = vec![0; HEADER_BYTES];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..12].copy_from_slice(&version.to_le_bytes());
    bytes[16..24].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(body.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&body);
    reseal(&mut bytes);
    bytes
}

fn reseal(bytes: &mut [u8]) {
    let digest =
        Sha256::new().chain_update(&bytes[..32]).chain_update(&bytes[HEADER_BYTES..]).finalize();
    bytes[32..HEADER_BYTES].copy_from_slice(&digest);
}

/// Payload of one 1×1 document without layers, followed by `selection` bytes.
fn one_document(selection: &[u8]) -> Vec<u8> {
    let mut payload = vec![0, 0, 0, 0, 1, 0, 0, 0, 1, 0, b'k', 1, 0, 0, 0, 1, 0, 0, 0, 0];
    payload.extend_from_slice(selection);
    payload
}

#[test]
fn archives_round_trip_every_layer_kind_coverage_tile_and_selection_exactly() -> harness::TestResult
{
    run(|context| {
        let archive = rich_archive(context)?;
        let (bytes, _claim) = archive.encode(context)?;
        assert!(bytes.len() as u64 <= archive.encoded_bound());
        let decoded = InputDocumentArchive::decode(&bytes, context.memory(), context)?;
        assert_eq!(decoded, archive);
        assert_eq!(decoded.host_metadata(), archive.host_metadata());
        let keys = decoded.documents().iter().map(ArchivedDocument::key).collect::<Vec<_>>();
        assert_eq!(keys, ["empty", "workflow-a/node-9", "workflow-b/node-2"]);
        let original = archive.document("workflow-b/node-2").ok_or(ArchiveError::Truncated)?;
        let restored = decoded.document("workflow-b/node-2").ok_or(ArchiveError::Truncated)?;
        for (left, right) in original.layers().iter().zip(restored.layers()) {
            assert_eq!(left, right);
            assert_eq!(
                left.mask().map(Coverage::fingerprint),
                right.mask().map(Coverage::fingerprint)
            );
            assert_eq!(restored.source(right.id()), original.source(left.id()));
        }
        assert!(restored.source(LayerId::new(11).map_err(canvas)?).is_some());
        assert!(restored.source(LayerId::new(13).map_err(canvas)?).is_none());
        assert_eq!(
            restored.selection().map(Coverage::fingerprint),
            original.selection().map(Coverage::fingerprint)
        );
        assert_eq!(decoded.stored_tile_bytes(), archive.stored_tile_bytes());
        let all = decoded.document("workflow-a/node-9").and_then(ArchivedDocument::selection);
        assert_eq!(all.and_then(Coverage::uniform), Some(FULL_COVERAGE));
        assert_eq!(decoded.encode(context)?.0, bytes, "encoding is deterministic");
        assert_eq!(decoded.fingerprint(), archive.fingerprint());
        let renamed = InputDocumentArchive::new(archive.documents().to_vec(), b"other")?;
        assert_ne!(renamed.fingerprint(), archive.fingerprint(), "metadata is content");
        let mut fewer = archive.documents().to_vec();
        fewer.pop();
        let fewer = InputDocumentArchive::new(fewer, archive.host_metadata())?;
        assert_ne!(fewer.fingerprint(), archive.fingerprint());
        Ok(())
    })
}

#[test]
fn restored_documents_start_without_history_at_their_archived_content() -> harness::TestResult {
    run(|context| {
        let mut original = rich_document(context)?;
        let archived = ArchivedDocument::capture("doc", &original.snapshot(), source)?;
        let mut restored = archived.restore(42, context.memory()).map_err(canvas)?;
        let snapshot = restored.snapshot();
        assert_eq!((snapshot.identity(), snapshot.revision()), (42, 1));
        assert_eq!(snapshot.layers(), original.snapshot().layers());
        assert_eq!(snapshot.selection(), original.snapshot().selection());
        assert!(!restored.undo().map_err(canvas)?, "archives carry no history");
        assert!(original.undo().map_err(canvas)?);
        Ok(())
    })
}

#[test]
fn archive_content_outside_its_bounds_is_refused_before_encoding() -> harness::TestResult {
    run(|context| {
        let document = rich_document(context)?;
        let snapshot = document.snapshot();
        assert!(matches!(
            ArchivedDocument::capture("doc", &snapshot, |_| None),
            Err(ArchiveError::Invalid(_))
        ));
        assert!(ArchivedDocument::capture("", &snapshot, source).is_err());
        assert!(ArchivedDocument::capture(&"k".repeat(257), &snapshot, source).is_err());
        assert!(SourceReference::new(&vec![0; MAX_SOURCE_REFERENCE_BYTES + 1]).is_err());
        let archived = ArchivedDocument::capture("doc", &snapshot, source)?;
        let duplicate = vec![archived.clone(), archived.clone()];
        assert!(InputDocumentArchive::new(duplicate, &[]).is_err());
        let metadata = vec![0; MAX_HOST_METADATA_BYTES + 1];
        assert!(InputDocumentArchive::new(vec![archived], &metadata).is_err());
        Ok(())
    })
}

#[test]
fn truncated_flipped_oversized_and_unknown_archives_are_rejected() -> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let (bytes, _claim) = rich_archive(context)?.encode(context)?;
        for length in [0, 11, 40, HEADER_BYTES, bytes.len() - 1] {
            assert!(
                matches!(
                    InputDocumentArchive::decode(&bytes[..length], memory, context),
                    Err(ArchiveError::Truncated)
                ),
                "truncated at {length}"
            );
        }
        for position in [0, 13, 17, 26, 40, HEADER_BYTES + 3, bytes.len() / 2, bytes.len() - 1] {
            let mut flipped = bytes.clone();
            flipped[position] ^= 0x10;
            let result = InputDocumentArchive::decode(&flipped, memory, context);
            assert!(
                matches!(result, Err(ArchiveError::Corrupt(_) | ArchiveError::Truncated)),
                "bit flip at {position}: {result:?}"
            );
        }
        let mut future = bytes.clone();
        future[8] = 3;
        assert!(matches!(
            InputDocumentArchive::decode(&future, memory, context),
            Err(ArchiveError::UnsupportedVersion(3))
        ));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            InputDocumentArchive::decode(&trailing, memory, context),
            Err(ArchiveError::Corrupt(_))
        ));
        let mut huge = bytes.clone();
        huge[16..24].copy_from_slice(&(MAX_PAYLOAD_BYTES + 1).to_le_bytes());
        reseal(&mut huge);
        assert!(matches!(
            InputDocumentArchive::decode(&huge, memory, context),
            Err(ArchiveError::Oversized)
        ));
        let mut short = bytes;
        let declared = u64::from_le_bytes(field8(&short[16..24])) - 1;
        short[16..24].copy_from_slice(&declared.to_le_bytes());
        reseal(&mut short);
        assert!(matches!(
            InputDocumentArchive::decode(&short, memory, context),
            Err(ArchiveError::Corrupt(_))
        ));
        Ok(())
    })
}

fn field8(bytes: &[u8]) -> [u8; 8] {
    let mut value = [0; 8];
    value.copy_from_slice(bytes);
    value
}

#[test]
fn sealed_archives_with_invalid_content_are_rejected() -> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let decode = |bytes: &[u8]| InputDocumentArchive::decode(bytes, memory, context);
        assert!(decode(&seal(1, &one_document(&[0]))).is_ok());
        assert!(matches!(
            decode(&seal(7, &one_document(&[0]))),
            Err(ArchiveError::UnsupportedVersion(7))
        ));
        let cases: [(&str, Vec<u8>); 6] = [
            ("unknown selection tag", one_document(&[2])),
            ("payload continues", one_document(&[0, 0])),
            (
                "uniform tile repeats the default",
                one_document(&[1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ),
            (
                "tile off the grid",
                one_document(&[1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0]),
            ),
            ("more tiles than the grid", one_document(&[1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0])),
            ("too many documents", vec![0, 0, 0, 0, 1, 1, 0, 0]),
        ];
        for (case, payload) in cases {
            let result = decode(&seal(1, &payload));
            assert!(matches!(result, Err(ArchiveError::Corrupt(_))), "{case}: {result:?}");
        }
        let mut unsorted = vec![0, 0, 0, 0, 2, 0, 0, 0];
        for key in [b'b', b'a'] {
            unsorted.extend_from_slice(&[1, 0, key, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
        }
        assert!(matches!(decode(&seal(1, &unsorted)), Err(ArchiveError::Corrupt(_))));
        Ok(())
    })
}

#[test]
fn streamed_integrity_checks_agree_with_whole_archive_checks() -> harness::TestResult {
    run(|context| {
        let (bytes, _claim) = rich_archive(context)?.encode(context)?;
        let mut integrity = ArchiveIntegrity::new();
        for chunk in bytes.chunks(7) {
            integrity.update(chunk)?;
        }
        assert_eq!(integrity.finish()?, ArchiveIntegrity::check(&bytes)?);
        let mut extended = ArchiveIntegrity::new();
        extended.update(&bytes)?;
        assert!(matches!(extended.update(&[0]), Err(ArchiveError::Corrupt(_))));
        let mut foreign = ArchiveIntegrity::new();
        assert!(matches!(foreign.update(b"PNG\r\n\x1a\n\0"), Err(ArchiveError::Corrupt(_))));
        Ok(())
    })
}

#[test]
fn coverage_beyond_the_document_round_trips_and_version_one_archives_migrate() -> harness::TestResult
{
    run(|context| {
        let memory = context.memory();
        let extent = Extent::new(64, 48)?;
        let mut document = Document::new(3, extent, memory).map_err(canvas)?;
        let square = Path::rectangle([40.0, 10.0, 60.0, 30.0])?.rasterize(extent, context)?;
        let moved = square.translate([20, -15], context)?;
        assert!(moved.outside().is_some());
        let appearance = MaskAppearance::new([1, 2, 3], 0.5).map_err(canvas)?;
        let content = LayerContent::Coverage { coverage: moved, appearance };
        let layer = Layer::new(LayerId::new(5).map_err(canvas)?, content).map_err(canvas)?;
        document.apply(DocumentEdit::Insert { index: 0, layer }).map_err(canvas)?;
        let captured = ArchivedDocument::capture("k", &document.snapshot(), |_| None)?;
        let archive = InputDocumentArchive::new(vec![captured], b"")?;
        let (bytes, _claim) = archive.encode(context)?;
        assert_eq!(u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]), 2);
        let decoded = InputDocumentArchive::decode(&bytes, memory, context)?;
        assert_eq!(decoded, archive);
        let restored = match decoded.documents()[0].layers()[0].content() {
            LayerContent::Coverage { coverage, .. } => coverage.translate([-20, 15], context)?,
            _ => return Err(ArchiveError::Invalid("Mask layer lost")),
        };
        assert_eq!(restored, square);
        // Version 1 coverage has no beyond field; version 2 requires one.
        let decode = |bytes: &[u8]| InputDocumentArchive::decode(bytes, memory, context);
        let selection = [1, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0];
        let migrated = decode(&seal(1, &one_document(&selection)))?;
        assert!(migrated.documents()[0].selection().is_some());
        assert!(decode(&seal(2, &one_document(&selection))).is_err());
        let mut tagged = selection.to_vec();
        tagged.push(0);
        assert_eq!(decode(&seal(2, &one_document(&tagged)))?, migrated);
        // A selection never keeps content beyond the document.
        let mut beyond = selection.to_vec();
        beyond.extend_from_slice(&[1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // One pixel of full coverage, canonical, just right of the document.
        beyond.extend_from_slice(&[1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        beyond.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff]);
        assert!(matches!(decode(&seal(2, &one_document(&beyond))), Err(ArchiveError::Corrupt(_))));
        Ok(())
    })
}
