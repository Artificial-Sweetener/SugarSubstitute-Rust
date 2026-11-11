//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Provide a least-recently-used map whose retained bytes count toward the page allowance. */
import type { MemoryBudgetRegistry, MemoryConsumer, MemoryTier } from './MemoryBudgetRegistry.js';

export interface BudgetedMemoryCacheOptions<Key, Value> {
  /** Unique registry identity. */
  readonly id: string;
  readonly registry: MemoryBudgetRegistry;
  /** Release platform resources whenever an entry leaves the cache for any reason. */
  readonly release?: (value: Value, key: Key) => void;
  /** Report entries that are visible or in use; they are never evicted. */
  readonly pinned?: (value: Value, key: Key) => boolean;
  /** Bound the entry count independently of the page allowance. */
  readonly maxEntries?: number;
  /** Bound retained bytes independently of the page allowance. */
  readonly maxBytes?: number;
}

/** Describe how one entry participates in eviction. */
export interface BudgetedEntry {
  readonly tier: MemoryTier;
  readonly bytes: number;
}

interface Slot<Value> {
  value: Value;
  tier: MemoryTier;
  bytes: number;
  used: number;
}

/**
 * Retain values under one registry consumer.
 *
 * Map order is use order, so the first unpinned entry of a tier is its least recently used.
 * Local bounds and the shared allowance both evict only unpinned entries.
 */
export class BudgetedMemoryCache<Key, Value> implements MemoryConsumer {
  readonly id: string;
  readonly #options: BudgetedMemoryCacheOptions<Key, Value>;
  readonly #slots = new Map<Key, Slot<Value>>();
  readonly #unregister: () => void;
  #bytes = 0;

  /** Register with the page allowance for the cache's whole lifetime. */
  constructor(options: BudgetedMemoryCacheOptions<Key, Value>) {
    this.id = options.id;
    this.#options = options;
    this.#unregister = options.registry.register(this);
  }

  /** Count retained entries. */
  get size(): number {
    return this.#slots.size;
  }

  /** Return a value and mark it most recently used. */
  get(key: Key): Value | undefined {
    const slot = this.#slots.get(key);
    if (!slot) return undefined;
    this.#slots.delete(key);
    slot.used = this.#options.registry.stamp();
    this.#slots.set(key, slot);
    return slot.value;
  }

  /** Return whether a key is retained without changing its use order. */
  has(key: Key): boolean {
    return this.#slots.has(key);
  }

  /** Retain one value, releasing a different value it replaces, then enforce every bound. */
  set(key: Key, value: Value, entry: BudgetedEntry): void {
    const previous = this.#slots.get(key);
    if (previous) {
      this.#slots.delete(key);
      this.#bytes -= previous.bytes;
      if (previous.value !== value) this.#options.release?.(previous.value, key);
    }
    const bytes = requireEntryBytes(entry.bytes);
    this.#slots.set(key, {
      value,
      tier: entry.tier,
      bytes,
      used: this.#options.registry.stamp(),
    });
    this.#bytes += bytes;
    this.#enforceLocalBounds();
    this.#options.registry.reconcile();
  }

  /** Update one retained entry's size once its real cost is known. */
  resize(key: Key, bytes: number): void {
    const slot = this.#slots.get(key);
    if (!slot) return;
    const next = requireEntryBytes(bytes);
    this.#bytes += next - slot.bytes;
    slot.bytes = next;
    this.#enforceLocalBounds();
    this.#options.registry.reconcile();
  }

  /** Remove and release one entry. */
  delete(key: Key): boolean {
    const slot = this.#slots.get(key);
    if (!slot) return false;
    this.#remove(key, slot);
    return true;
  }

  /** Remove one entry without releasing it, transferring ownership of its value to the caller. */
  take(key: Key): Value | undefined {
    const slot = this.#slots.get(key);
    if (!slot) return undefined;
    this.#slots.delete(key);
    this.#bytes -= slot.bytes;
    return slot.value;
  }

  /** Remove and release every entry matching a predicate. */
  deleteWhere(matches: (key: Key, value: Value) => boolean): void {
    for (const [key, slot] of [...this.#slots])
      if (matches(key, slot.value)) this.#remove(key, slot);
  }

  /** Remove and release every entry. */
  clear(): void {
    for (const [key, slot] of [...this.#slots]) this.#remove(key, slot);
  }

  /** Release every entry and leave the page allowance. */
  dispose(): void {
    this.clear();
    this.#unregister();
  }

  /** Report every retained byte, pinned entries included. */
  bytes(): number {
    return this.#bytes;
  }

  /** Report the least recently used evictable entry of one tier. */
  oldestEvictable(tier: MemoryTier): number | null {
    for (const [key, slot] of this.#slots)
      if (slot.tier === tier && !this.#pinned(key, slot)) return slot.used;
    return null;
  }

  /** Evict least recently used unpinned entries of one tier until `targetBytes` are released. */
  evict(targetBytes: number, tier: MemoryTier): number {
    let released = 0;
    for (const [key, slot] of [...this.#slots]) {
      if (released >= targetBytes) break;
      if (slot.tier !== tier || this.#pinned(key, slot)) continue;
      released += slot.bytes;
      this.#remove(key, slot);
    }
    return released;
  }

  /** Apply the cache's own entry and byte bounds, oldest unpinned entries first. */
  #enforceLocalBounds(): void {
    const { maxEntries = Number.POSITIVE_INFINITY, maxBytes = Number.POSITIVE_INFINITY } =
      this.#options;
    for (const [key, slot] of [...this.#slots]) {
      if (this.#slots.size <= maxEntries && this.#bytes <= maxBytes) return;
      if (!this.#pinned(key, slot)) this.#remove(key, slot);
    }
  }

  /** Read one entry's in-use state from its owner. */
  #pinned(key: Key, slot: Slot<Value>): boolean {
    return this.#options.pinned?.(slot.value, key) ?? false;
  }

  /** Drop one entry and release its platform resources exactly once. */
  #remove(key: Key, slot: Slot<Value>): void {
    if (this.#slots.get(key) !== slot) return;
    this.#slots.delete(key);
    this.#bytes -= slot.bytes;
    this.#options.release?.(slot.value, key);
  }
}

/** Accept only finite non-negative entry sizes. */
function requireEntryBytes(bytes: number): number {
  if (!Number.isFinite(bytes) || bytes < 0)
    throw new RangeError('Cache entry size must be a non-negative byte count.');
  return bytes;
}
