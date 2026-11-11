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
/** Prove the page allowance evicts by tier and recency while protecting pinned entries. */
import { expect, test } from '@jest/globals';
import {
  DEFAULT_MEMORY_ALLOWANCE_BYTES,
  MemoryBudgetRegistry,
  type MemoryTier,
} from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { BudgetedMemoryCache } from '../../../frontend/comfyui/ui/core/memory/BudgetedMemoryCache.js';

const MIB = 1024 * 1024;

/** Build one cache whose released and pinned keys are observable. */
function cache(registry: MemoryBudgetRegistry, id: string, pinned = new Set<string>()) {
  const released: string[] = [];
  const store = new BudgetedMemoryCache<string, string>({
    id,
    registry,
    pinned: (_value, key) => pinned.has(key),
    release: (_value, key) => released.push(key),
  });
  return { store, released, pinned };
}

/** Insert one entry of a given tier and size. */
function put(
  store: BudgetedMemoryCache<string, string>,
  key: string,
  tier: MemoryTier,
  mib: number,
) {
  store.set(key, key, { tier, bytes: mib * MIB });
}

test('defaults to the standalone allowance and reports totals per consumer', () => {
  const registry = new MemoryBudgetRegistry();
  const images = cache(registry, 'images');
  const data = cache(registry, 'data');
  put(images.store, 'a', 2, 3);
  put(data.store, 'b', 3, 5);
  expect(DEFAULT_MEMORY_ALLOWANCE_BYTES).toBe(192 * MIB);
  expect(registry.usage()).toEqual({
    usedBytes: 8 * MIB,
    consumers: [
      { id: 'images', bytes: 3 * MIB },
      { id: 'data', bytes: 5 * MIB },
    ],
  });
  images.store.dispose();
  expect(registry.usage().consumers.map((consumer) => consumer.id)).toEqual(['data']);
});

test('evicts the lowest tier first and the least recently used entry within a tier', () => {
  const registry = new MemoryBudgetRegistry(10 * MIB);
  const derived = cache(registry, 'derived');
  const decoded = cache(registry, 'decoded');
  const fetched = cache(registry, 'fetched');
  put(fetched.store, 'fetched', 3, 2);
  put(decoded.store, 'decoded-old', 2, 2);
  put(derived.store, 'derived-old', 1, 2);
  put(decoded.store, 'decoded-new', 2, 2);
  put(derived.store, 'derived-new', 1, 2);
  expect(registry.usage().usedBytes).toBe(10 * MIB);
  decoded.store.get('decoded-old');

  put(fetched.store, 'incoming', 3, 5);

  expect(derived.released).toEqual(['derived-old', 'derived-new']);
  expect(decoded.released).toEqual(['decoded-new']);
  expect(fetched.released).toEqual([]);
  expect(registry.usage().usedBytes).toBe(9 * MIB);
});

test('never evicts pinned entries, even when they alone exceed the allowance', () => {
  const registry = new MemoryBudgetRegistry(4 * MIB);
  const visible = cache(registry, 'visible', new Set(['shown']));
  put(visible.store, 'shown', 1, 6);
  put(visible.store, 'hidden', 1, 1);
  expect(visible.released).toEqual(['hidden']);
  expect(visible.store.has('shown')).toBe(true);
  registry.setAllowance(4 * MIB, 'critical');
  expect(visible.store.has('shown')).toBe(true);
  visible.pinned.delete('shown');
  registry.reconcile();
  expect(visible.store.size).toBe(0);
});

test('applies each pressure level target and holds its steady ceiling', () => {
  const registry = new MemoryBudgetRegistry(100 * MIB);
  const images = cache(registry, 'images');
  for (let index = 0; index < 10; index++) put(images.store, `image-${String(index)}`, 2, 10);
  expect(registry.usage().usedBytes).toBe(100 * MIB);

  registry.setAllowance(100 * MIB, 'elevated');
  expect(registry.usage().usedBytes).toBe(80 * MIB);
  put(images.store, 'grow', 2, 10);
  expect(registry.usage().usedBytes).toBeLessThanOrEqual(85 * MIB);

  registry.setAllowance(200 * MIB, 'high');
  expect(registry.usage().usedBytes).toBe(40 * MIB);
  for (let index = 0; index < 10; index++) put(images.store, `again-${String(index)}`, 2, 10);
  expect(registry.usage().usedBytes).toBe(100 * MIB);

  registry.setAllowance(200 * MIB, 'normal');
  for (let index = 0; index < 20; index++) put(images.store, `normal-${String(index)}`, 2, 10);
  expect(registry.usage().usedBytes).toBe(200 * MIB);

  registry.setAllowance(200 * MIB, 'critical');
  expect(registry.usage().usedBytes).toBe(0);
});

test('publishes one coalesced usage report per change burst', async () => {
  const registry = new MemoryBudgetRegistry();
  const images = cache(registry, 'images');
  const reports: number[] = [];
  const stop = registry.subscribe((usage) => reports.push(usage.usedBytes));
  put(images.store, 'a', 2, 1);
  put(images.store, 'b', 2, 1);
  await Promise.resolve();
  stop();
  put(images.store, 'c', 2, 1);
  await Promise.resolve();
  expect(reports).toEqual([2 * MIB]);
});

test('applies local entry and byte bounds independently of the allowance', () => {
  const registry = new MemoryBudgetRegistry();
  const released: string[] = [];
  const store = new BudgetedMemoryCache<string, string>({
    id: 'bounded',
    registry,
    maxEntries: 2,
    maxBytes: 3 * MIB,
    release: (_value, key) => released.push(key),
  });
  store.set('a', 'a', { tier: 3, bytes: MIB });
  store.set('b', 'b', { tier: 3, bytes: MIB });
  store.set('c', 'c', { tier: 3, bytes: MIB });
  store.set('d', 'd', { tier: 3, bytes: 2 * MIB });
  expect(released).toEqual(['a', 'b']);
  expect(store.bytes()).toBe(3 * MIB);
  expect(() => registry.setAllowance(-1, 'normal')).toThrow(RangeError);
});
