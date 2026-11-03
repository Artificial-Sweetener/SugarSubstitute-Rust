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
