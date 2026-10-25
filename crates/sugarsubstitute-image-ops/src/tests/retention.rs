// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared backend retention preserves pinned allocations and bounded level fairness.
use super::*;

#[test]
fn shared_tile_retention_releases_only_unpinned_entries_under_memory_pressure() -> TestResult {
    run(MEMORY, |context| {
        let mut cache = TileRetention::new(2, context.memory())?;
        let region = Region::new(0, 0, 1, 1)?;
        let pinned = Arc::new(context.memory().claim(4096)?);
        let other = Arc::new(context.memory().claim(8192)?);
        cache.insert([1; 32], region, Some(0), pinned.clone());
        cache.insert([2; 32], region, Some(0), other.clone());
        assert!(!cache.evict_unpinned(|_| true));
        drop(other);
        assert!(!cache.evict_unpinned(|_| false));
        assert!(cache.evict_unpinned(|_| true));
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(pinned.bytes(), 4096);
        assert!(cache.find([1; 32], region, Some(0)).is_none());
        Ok(())
    })
}

#[test]
fn shared_tile_retention_preserves_content_region_and_extreme_level_bounds() -> TestResult {
    run(MEMORY, |context| {
        assert!(TileRetention::<u32>::new(0, context.memory()).is_err());
        assert!(TileRetention::<u32>::new(4097, context.memory()).is_err());
        let mut cache = TileRetention::new(2, context.memory())?;
        let region = Region::new(7, 11, 1, 1)?;
        cache.prefer_level(u8::MAX);
        cache.insert([1; 32], region, Some(u8::MAX), Arc::new(17));
        cache.insert([2; 32], region, Some(0), Arc::new(23));
        cache.insert([3; 32], region, None, Arc::new(29));
        assert_eq!(cache.len(), 2);
        assert!(cache.find([2; 32], region, Some(0)).is_none());
        assert_eq!(cache.find([1; 32], region, Some(u8::MAX)).as_deref(), Some(&17));
        assert!(cache.find([1; 32], Region::new(8, 11, 1, 1)?, None).is_none());
        Ok(())
    })
}
