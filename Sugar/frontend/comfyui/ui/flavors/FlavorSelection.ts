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
/**
 * Own the SugarCubes flavor-selection helpers in
 * `frontend/comfyui/ui/flavors/FlavorSelection.js`.
 */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

export type FlavorScope = 'authored' | 'local';

export interface FlavorOption {
  id: string;
  name: string;
  scope: FlavorScope;
  stale: boolean;
  values: UnknownRecord;
  updated_at?: unknown;
  selected?: boolean;
}

interface FlavorOptionsInput {
  authoredFlavors?: unknown;
  localFlavors?: unknown;
  selectedFlavorId?: unknown;
  selectedFlavorScope?: unknown;
}

function cloneValues(values: unknown): UnknownRecord {
  const cloned: unknown = JSON.parse(JSON.stringify(isRecord(values) ? values : {}));
  return isRecord(cloned) ? cloned : {};
}

/**
 * Normalize a machine-safe flavor id.
 */
export function normalizeFlavorId(value: unknown): string {
  return String(value || '')
    .trim()
    .toLowerCase()
    .replace(/[^0-9a-z_-]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

/**
 * Deduplicate a normalized flavor id against existing ids.
 */
export function dedupeFlavorId(
  baseId: unknown,
  usedIds: Iterable<unknown> | null | undefined,
): string {
  const normalizedBase = normalizeFlavorId(baseId) || 'flavor';
  const used =
    usedIds instanceof Set
      ? new Set(Array.from(usedIds, (value) => String(value)))
      : new Set(Array.from(usedIds || [], (value) => String(value)).filter(Boolean));
  if (!used.has(normalizedBase)) {
    return normalizedBase;
  }
  let suffix = 2;
  while (used.has(`${normalizedBase}_${suffix}`)) {
    suffix += 1;
  }
  return `${normalizedBase}_${suffix}`;
}

/**
 * Normalize authored flavor entries from persisted cube data.
 */
export function normalizeAuthoredFlavors(
  flavors: unknown,
  defaultFlavorId = 'default',
): FlavorOption[] {
  const normalized: FlavorOption[] = [];
  const usedIds = new Set<string>();
  const entries = Array.isArray(flavors) ? flavors : [];
  for (const entry of entries) {
    const record = isRecord(entry) ? entry : {};
    const rawId = typeof record.id === 'string' ? record.id : '';
    const rawName = typeof record.name === 'string' ? record.name : '';
    const id = rawId === 'default' ? 'default' : dedupeFlavorId(rawId || rawName, usedIds);
    const name = id === 'default' ? 'Default' : rawName.trim() || id;
    usedIds.add(id);
    normalized.push({
      id,
      name,
      scope: 'authored',
      stale: false,
      values: cloneValues(record.values),
    });
  }
  if (!normalized.some((entry) => entry.id === 'default')) {
    normalized.unshift({
      id: 'default',
      name: 'Default',
      scope: 'authored',
      stale: false,
      values: {},
    });
  }
  normalized.sort((left, right) => {
    if (left.id === defaultFlavorId || left.id === 'default') {
      return -1;
    }
    if (right.id === defaultFlavorId || right.id === 'default') {
      return 1;
    }
    return left.name.localeCompare(right.name);
  });
  return normalized;
}

/**
 * Build merged flavor options for one instance surface.
 */
export function buildFlavorOptions({
  authoredFlavors,
  localFlavors,
  selectedFlavorId,
  selectedFlavorScope,
}: FlavorOptionsInput = {}): FlavorOption[] {
  const authored = normalizeAuthoredFlavors(authoredFlavors);
  const localEntries = Array.isArray(localFlavors)
    ? localFlavors.map((entry): FlavorOption => {
        const record = isRecord(entry) ? entry : {};
        return {
          id: typeof record.id === 'string' ? record.id : '',
          name: typeof record.name === 'string' ? record.name : '',
          scope: 'local',
          stale: Boolean(record.stale),
          updated_at: record.updated_at || null,
          values: cloneValues(record.values),
        };
      })
    : [];
  const options = authored.concat(localEntries).filter((entry) => entry.id);
  return options.map((entry) => ({
    ...entry,
    selected: entry.id === selectedFlavorId && entry.scope === (selectedFlavorScope || entry.scope),
  }));
}

/**
 * Resolve the authored Default entry for defaults-only runtime behavior.
 */
export function defaultAuthoredFlavor(
  flavors: unknown,
  fallbackValues: unknown = {},
): FlavorOption {
  const authored = normalizeAuthoredFlavors(flavors, 'default');
  const defaultEntry = authored.find((entry) => entry.id === 'default') || null;
  return {
    id: 'default',
    name: 'Default',
    scope: 'authored',
    stale: false,
    selected: true,
    values: cloneValues(defaultEntry?.values || fallbackValues),
  };
}

/**
 * Return the single active option while broader flavor support is dormant.
 */
export function defaultsOnlyFlavorOptions(
  flavors: unknown,
  fallbackValues: unknown = {},
): FlavorOption[] {
  return [defaultAuthoredFlavor(flavors, fallbackValues)];
}

/**
 * Resolve the selected flavor entry.
 */
export function resolveSelectedFlavor(
  options: readonly FlavorOption[] | null | undefined,
  selectedFlavorId: unknown,
  selectedFlavorScope: unknown,
): FlavorOption | null {
  const entries = Array.isArray(options) ? options : [];
  return (
    entries.find(
      (entry) =>
        entry?.id === selectedFlavorId && entry?.scope === (selectedFlavorScope || entry?.scope),
    ) ||
    entries.find((entry) => entry?.id === 'default' && entry?.scope === 'authored') ||
    entries[0] ||
    null
  );
}
