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
/** Validate native seed modes for presentation and workflow restoration. */
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

export type SeedMode = 'fixed' | 'increment' | 'decrement' | 'randomize';

/** Narrow supported generation modes without accepting arbitrary combo choices. */
export function isSeedMode(value: unknown): value is SeedMode {
  return (
    value === 'fixed' || value === 'increment' || value === 'decrement' || value === 'randomize'
  );
}

/** Require the actual host combo contract and both primary-toggle choices. */
export function isComfySeedModeControl(value: unknown): value is UnknownRecord {
  if (!isRecord(value) || value.type !== 'combo' || !isSeedMode(value.value)) return false;
  const options = isRecord(value.options) ? value.options : {};
  const modes = options.values;
  return (
    Array.isArray(modes) &&
    modes.every(isSeedMode) &&
    modes.includes('fixed') &&
    modes.includes('randomize')
  );
}
