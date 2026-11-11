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
/** Validate durable workflow mask bindings before using native document references. */
import { isRecord } from '../../types/common.js';
import { isCanvasMaskReference } from '../../core/embedded/CanvasRuntime.js';
import { MASK_BINDING_KEY } from '../domain/InputBinding.js';
import type { CanvasMaskReference } from '../../../../../integrations/contracts/CanvasMasksProtocol.generated.js';
/** Preserve persisted version-one entries only at the migration boundary. */
export interface LegacyMaskEntry {
  readonly id: string;
  readonly label: string;
  readonly file: string | null;
  readonly mask: CanvasMaskReference | null;
}
/** Describe the public saved-workflow version-one contract. */
export interface LegacyMaskBinding {
  readonly version: 1;
  readonly field: string;
  readonly selected: string | null;
  readonly entries: readonly LegacyMaskEntry[];
  readonly files: readonly string[];
}

/** Read persisted bindings strictly so malformed entries cannot silently become another mask. */
export function readLegacyMaskBinding(properties: unknown): LegacyMaskBinding | null {
  if (!isRecord(properties) || !Object.hasOwn(properties, MASK_BINDING_KEY)) return null;
  const value = properties[MASK_BINDING_KEY];
  if (isRecord(value) && value.version !== 1) return null;
  if (
    !isRecord(value) ||
    value.version !== 1 ||
    typeof value.field !== 'string' ||
    value.field.length > 128 ||
    !Array.isArray(value.entries) ||
    !Array.isArray(value.files) ||
    value.files.length > 64 ||
    value.files.some((file: unknown) => typeof file !== 'string' || file.length > 4096) ||
    value.entries.length > 64 ||
    (value.selected !== null && typeof value.selected !== 'string')
  )
    throw new Error('Invalid canvas mask binding.');
  const entries: LegacyMaskEntry[] = [];
  const ids = new Set<string>();
  for (const entry of value.entries) {
    if (
      !isRecord(entry) ||
      typeof entry.id !== 'string' ||
      !entry.id ||
      entry.id.length > 128 ||
      ids.has(entry.id) ||
      typeof entry.label !== 'string' ||
      entry.label.length > 128 ||
      (entry.file !== null && typeof entry.file !== 'string') ||
      (typeof entry.file === 'string' && entry.file.length > 4096) ||
      (entry.mask !== null && !isCanvasMaskReference(entry.mask))
    )
      throw new Error('Invalid canvas mask entry.');
    ids.add(entry.id);
    entries.push({ id: entry.id, label: entry.label, file: entry.file, mask: entry.mask });
  }
  if (value.selected !== null && !ids.has(value.selected))
    throw new Error('Invalid selected canvas mask.');
  return {
    version: 1,
    field: value.field,
    selected: value.selected,
    entries,
    files: value.files.filter((file): file is string => typeof file === 'string'),
  };
}
