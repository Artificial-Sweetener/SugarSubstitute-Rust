// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact tile-level coverage export and canonical import.
use super::*;

const MEMORY: u64 = 16 * 1024 * 1024;

/// Rebuild a coverage from its own exported tiles.
fn rebuild(coverage: &Coverage, context: &JobContext) -> Result<Coverage, ImageError> {
    let mut builder = CoverageBuilder::new(coverage.extent(), coverage.default_value());
    for (index, values) in coverage.tile_values() {
        match values {
            CoverageTileValues::Uniform(value) => builder.uniform(index, value)?,
            CoverageTileValues::Stored(values) => {
                builder.stored(index, values.to_vec(), context.memory())?;
            }
        }
    }
    builder.finish(context.memory())
}

#[test]
fn exported_tiles_rebuild_identical_sparse_uniform_and_stored_coverage() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let empty = Coverage::new(extent, 0, context.memory())?;
        assert_eq!(empty.tile_count(), 6);
        assert_eq!(empty.tile_values().count(), 0);
        assert_eq!(rebuild(&empty, context)?, empty);
        let full = Coverage::new(extent, FULL_COVERAGE, context.memory())?;
        assert_eq!(rebuild(&full, context)?.uniform(), Some(FULL_COVERAGE));
        let mut painted = empty.clone();
        painted.update(Region::new(0, 0, 128, 128)?, context.memory(), |_, _, _| 9000)?;
        painted.update(Region::new(260, 150, 40, 50)?, context.memory(), |x, y, _| {
            (x * 31 + y * 7) as u16
        })?;
        let exported = painted.tile_values().collect::<Vec<_>>();
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0], (0, CoverageTileValues::Uniform(9000)));
        let CoverageTileValues::Stored(edge) = exported[1].1 else {
            return Err(ImageError::Invalid("The partial edge tile is stored"));
        };
        assert_eq!(exported[1].0, 5);
        assert_eq!(edge.len(), CoverageBuilder::stored_tile_values());
        assert_eq!(edge[44 * 128 + 44], 0, "positions outside the extent stay zero");
        let rebuilt = rebuild(&painted, context)?;
        assert_eq!(rebuilt, painted);
        assert_eq!(rebuilt.fingerprint(), painted.fingerprint());
        assert_eq!(rebuilt.value(299, 199), painted.value(299, 199));
        assert_eq!(rebuilt.stored_bytes(), painted.stored_bytes());
        let inverted = painted.invert(context.memory())?;
        assert_eq!(rebuild(&inverted, context)?, inverted);
        Ok(())
    })
}

#[test]
fn coverage_import_rejects_noncanonical_and_malformed_tiles() -> TestResult {
    run(MEMORY, |context| {
        let memory = context.memory();
        let extent = Extent::new(300, 200)?;
        let mut builder = CoverageBuilder::new(extent, 0);
        assert!(builder.uniform(0, 0).is_err(), "a uniform tile never repeats the default");
        builder.uniform(1, 5)?;
        assert!(builder.uniform(1, 6).is_err(), "indices strictly ascend");
        assert!(builder.uniform(0, 6).is_err(), "indices strictly ascend");
        assert!(builder.uniform(6, 6).is_err(), "indices stay on the grid");
        let tile = CoverageBuilder::stored_tile_values();
        assert!(builder.stored(2, vec![7; tile - 1], memory).is_err(), "wrong value count");
        let mut builder = CoverageBuilder::new(extent, 0);
        assert!(builder.stored(0, vec![7; tile], memory).is_err(), "uniform values are not stored");
        let mut outside = vec![0; tile];
        outside[0] = 1;
        outside[127] = 3;
        let mut builder = CoverageBuilder::new(extent, 0);
        assert!(
            builder.stored(2, outside.clone(), memory).is_err(),
            "the right edge tile covers only 44 columns"
        );
        let mut builder = CoverageBuilder::new(extent, 0);
        builder.stored(0, outside, memory)?;
        let coverage = builder.finish(memory)?;
        assert_eq!((coverage.value(0, 0), coverage.value(127, 0), coverage.value(1, 0)), (1, 3, 0));
        Ok(())
    })
}

/// A document's lease in tests: a fixed reservation whose final release needs no owner.
struct Unowned;
impl sugarsubstitute_contracts::execution::MemoryRelease for Unowned {
    fn release(&self, _: u64) {}
}

/// Pixels of the document the reservation regression filters.
const RESERVED_EDGE: u32 = 1024;
/// The canvas's per-job reservation for coverage work: six bytes per document pixel and 1 MiB.
const RESERVED_JOB: u64 = RESERVED_EDGE as u64 * RESERVED_EDGE as u64 * 6 + (1 << 20);

#[test]
fn filters_of_coverage_spanning_the_document_fit_a_six_byte_per_pixel_job() -> TestResult {
    run(RESERVED_JOB, |context| {
        let document =
            sugarsubstitute_contracts::execution::MemoryLease::new(64 << 20, Box::new(Unowned));
        let extent = Extent::new(RESERVED_EDGE, RESERVED_EDGE)?;
        // A soft mask reaching every edge, as a painted or loaded mask commonly does, so each
        // filter works over the whole document.
        let mut mask = Coverage::new(extent, 0, &document)?;
        mask.update(extent.region(), &document, |x, y, _| ((x * 37 + y * 11) % 65_536) as u16)?;
        let feathered = mask.feather(12.0, context)?;
        assert_ne!(feathered, mask);
        drop(feathered);
        assert_eq!(context.memory().claimed_bytes(), 0);
        let expanded = mask.expand(12, context)?;
        assert_ne!(expanded, mask);
        drop(expanded);
        assert_eq!(context.memory().claimed_bytes(), 0);
        Ok(())
    })
}

#[test]
fn adopted_coverage_moves_its_charge_to_the_keeping_lease_and_keeps_shared_tiles_intact()
-> TestResult {
    run(MEMORY, |context| {
        let document =
            sugarsubstitute_contracts::execution::MemoryLease::new(MEMORY, Box::new(Unowned));
        let extent = Extent::new(300, 200)?;
        let base = Coverage::new(extent, 0, &document)?;
        let mut kept = base.clone();
        kept.update(Region::new(0, 0, 100, 100)?, &document, |x, _, _| x as u16)?;
        // A job paints one tile of its own beside the tile the document already charges.
        let mut result = kept.clone();
        result.update(Region::new(140, 10, 20, 20)?, context.memory(), |_, y, _| y as u16)?;
        let job_charged = context.memory().claimed_bytes();
        assert!(job_charged > 0 && !result.charged_to(&document));
        let painted = result.value(150, 20);
        let before = document.claimed_bytes();
        let adopted = result.clone().adopt(&document)?;
        assert_eq!(adopted, result);
        assert!(adopted.charged_to(&document));
        assert!(document.claimed_bytes() > before);
        // The job's copy is still shared by `result`, so its charge stays until that drops.
        drop(result);
        assert_eq!(context.memory().claimed_bytes(), 0);
        assert_eq!((adopted.value(50, 50), adopted.value(150, 20)), (50, painted));
        // Coverage already charged to the lease adopts without any new charge.
        let charged = document.claimed_bytes();
        let again = adopted.clone().adopt(&document)?;
        assert_eq!(document.claimed_bytes(), charged);
        assert_eq!(again, adopted);
        // A lease too small for the move refuses it as a memory failure.
        let small = sugarsubstitute_contracts::execution::MemoryLease::new(64, Box::new(Unowned));
        assert!(matches!(again.adopt(&small), Err(ImageError::Memory(_))));
        assert_eq!(small.claimed_bytes(), 0);
        Ok(())
    })
}
