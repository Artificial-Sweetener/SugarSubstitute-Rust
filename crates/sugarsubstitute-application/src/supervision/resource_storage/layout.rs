// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Conservative Global allocation layouts for the pinned Rust 1.93.1 collections.
use crate::supervision::resource_state::REQUEST_HISTORY;
use sugarsubstitute_contracts::lifecycle::LifecycleRejection;

const BTREE_SLOTS: usize = 11;
const BTREE_EDGES: usize = 12;
const NODE_PADDING_FIELDS: usize = 7;
const RAWVEC_MAX_INITIAL_CAPACITY: usize = 8;

pub(super) type Bytes = Result<usize, LifecycleRejection>;

pub(super) fn add(left: usize, right: usize) -> Bytes {
    left.checked_add(right).ok_or(LifecycleRejection::Overloaded)
}

pub(super) fn multiply(left: usize, right: usize) -> Bytes {
    left.checked_mul(right).ok_or(LifecycleRejection::Overloaded)
}

/// One internal-node allocation upper bound, including leaf/header/edge padding.
/// Rust1.93.1 alloc/collections/btree/node.rs uses11 key/value slots,12 pointers,
/// one parent pointer and two u16 fields. Seven maximum-align padding allowances
/// cover five leaf fields and the two-field enclosing internal node in any order.
/// Box/Global requests that layout without a Rust allocation prefix. OS allocator
/// bookkeeping/rounding is opaque and excluded; this is not a process-peak bound.
pub(super) fn node<K, V>() -> Bytes {
    let alignment = align_of::<K>().max(align_of::<V>()).max(align_of::<usize>());
    let header = add(size_of::<usize>(), multiply(2, size_of::<u16>())?)?;
    let entries = multiply(BTREE_SLOTS, add(size_of::<K>(), size_of::<V>())?)?;
    let edges = multiply(BTREE_EDGES, size_of::<usize>())?;
    add(add(add(header, entries)?, edges)?, multiply(NODE_PADDING_FIELDS, alignment)?)
}

/// Budget both retained and transient nodes without assuming dense occupancy.
/// Every retained nonempty node owns a key; an empty root adds at most one.
/// Insertion/bulk-build's active path can add at most another entries+1 nodes.
/// This deliberately loose two-tree bound also covers deletion's retained empty
/// root. Private node layouts are never copied into the Host or constructed here.
pub(super) fn tree<K, V>(entries: usize) -> Bytes {
    multiply(multiply(add(entries, 1)?, 2)?, node::<K, V>()?)
}

pub(super) fn tree_growth<K, V>(entries: usize) -> Bytes {
    multiply(multiply(entries, 2)?, node::<K, V>()?)
}

/// Cover collect/push geometric capacity and a moving reallocation overlap.
/// The pinned RawVec grows by max(2*capacity,required,min_non_zero_cap), at most8
/// initial elements. Twice the final length plus8 is conservative for these
/// bounded pushes/collects. Factor2 additionally funds old/new buffer overlap.
pub(super) fn vector<T>(elements: usize) -> Bytes {
    if elements == 0 || size_of::<T>() == 0 {
        return Ok(0);
    }
    multiply(
        multiply(add(multiply(elements, 2)?, RAWVEC_MAX_INITIAL_CAPACITY)?, 2)?,
        size_of::<T>(),
    )
}

/// A history push occurs before the oldest record is removed. It may require65
/// elements even when only64 records remain. Keep old capacity in the retained
/// sum and fund all new geometric buffers without subtracting freed storage.
pub(super) fn history_growth<T>(capacity: usize, len: usize, pushes: usize) -> Bytes {
    let required = add(len, pushes)?.min(add(REQUEST_HISTORY, 1)?);
    if pushes == 0 || required <= capacity {
        return Ok(0);
    }
    let minimum = if size_of::<T>() == 1 {
        8
    } else if size_of::<T>() <= 1024 {
        4
    } else {
        1
    };
    let mut next = capacity.max(minimum);
    while next < required {
        next = multiply(next, 2)?;
    }
    multiply(multiply(next, 2)?, size_of::<T>())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_overflow_rejects_without_wrapping() {
        assert_eq!(add(usize::MAX, 1), Err(LifecycleRejection::Overloaded));
        assert_eq!(multiply(usize::MAX, 2), Err(LifecycleRejection::Overloaded));
        assert_eq!(tree::<u64, u64>(usize::MAX), Err(LifecycleRejection::Overloaded));
        assert_eq!(vector::<u64>(usize::MAX), Err(LifecycleRejection::Overloaded));
    }
    #[test]
    fn private_node_header_edges_and_alignment_are_not_entry_size_only()
    -> Result<(), LifecycleRejection> {
        let layout = node::<u64, u64>()?;
        assert!(layout >= 11 * (size_of::<u64>() + size_of::<u64>()) + 12 * size_of::<usize>());
        assert_eq!(tree::<u64, u64>(256)?, 514 * layout);
        assert_eq!(tree_growth::<u64, u64>(256)?, 512 * layout);
        Ok(())
    }
    #[test]
    fn full_history_funds_push_before_pop_and_retained_spare_capacity()
    -> Result<(), LifecycleRejection> {
        assert!(history_growth::<u64>(64, 64, 1)? >= 128 * size_of::<u64>());
        assert_eq!(history_growth::<u64>(128, 64, 64)?, 0);
        assert_eq!(history_growth::<u64>(64, 64, 0)?, 0);
        Ok(())
    }
}
