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
/** Validate complete unknown resource replies before the sole request/cache owner publishes them. */
import { requireValue } from '../../../core/src/required.js';
import type { LoraResource } from './contracts.js';
import { loraItemKeys, loraRecord, readLoraItemFields } from './metadata-validation.js';
/** Immutable validated records retain actual metadata identity across resource reuse and dependent preparation. */
const validatedResources = new WeakSet<object>();
/** Verify one-to-one requested-name coverage, source ordering and immutable nested metadata without logging host values. */
export function readLoraResources(
  value: unknown,
  names: readonly string[],
): readonly LoraResource[] {
  if (!Array.isArray(value) || value.length !== names.length)
    throw new TypeError('Invalid LoRA resource count');
  const expected = new Set(names),
    results = new Map<string, LoraResource>();
  for (const item of value) {
    const resolution = item && typeof item === 'object' && 'resolution' in item && item.resolution;
    const raw = loraRecord(
      item,
      resolution === 'found'
        ? ['name', 'resolution', ...loraItemKeys]
        : resolution === 'ambiguous'
          ? ['name', 'resolution', 'candidateCount']
          : ['name', 'resolution'],
    );
    if (typeof raw.name !== 'string') throw new TypeError('Invalid LoRA resource identity');
    const name = raw.name;
    if (!expected.has(name) || results.has(name))
      throw new TypeError('Invalid LoRA resource identity');
    if (validatedResources.has(item)) {
      results.set(name, item as LoraResource);
      continue;
    }
    if (raw.resolution === 'found') {
      results.set(
        name,
        Object.freeze({
          name,
          resolution: 'found',
          ...readLoraItemFields(raw),
        }),
      );
    } else if (raw.resolution === 'ambiguous') {
      const count = raw.candidateCount;
      if (
        count !== null &&
        (typeof count !== 'number' || !Number.isSafeInteger(count) || count < 2)
      )
        throw new TypeError('Invalid LoRA ambiguity count');
      results.set(name, Object.freeze({ name, resolution: 'ambiguous', candidateCount: count }));
    } else if (
      raw.resolution === 'missing' ||
      raw.resolution === 'pending-authority' ||
      raw.resolution === 'unavailable'
    )
      results.set(name, Object.freeze({ name, resolution: raw.resolution }));
    else throw new TypeError('Invalid LoRA resource resolution');
  }
  return Object.freeze(
    names.map((name) => {
      const resource = requireValue(results.get(name), 'validation');
      validatedResources.add(resource);
      return resource;
    }),
  );
}
