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
/** Own additive portable Cube model requirements independently of the model catalog. */
import { isRecord } from '../types/common.js';

export interface ModelRequirement {
  readonly architecture: string;
  readonly family: string;
}

/** Accept future identity labels while rejecting malformed or unbounded persisted policy. */
export function decodeModelRequirement(value: unknown): ModelRequirement | null {
  if (value === undefined || value === null) return null;
  if (
    !isRecord(value) ||
    Object.keys(value).length !== 3 ||
    value.schemaVersion !== 1 ||
    typeof value.architecture !== 'string' ||
    typeof value.family !== 'string' ||
    [value.architecture, value.family].some(
      (label) => label.length > 128 || containsControlCharacter(label),
    )
  )
    throw new Error('Invalid Cube model requirement.');
  return { architecture: value.architecture.trim(), family: value.family.trim() };
}

/** Keep portable identities separate from machine-local model filenames. */
export function encodeModelRequirement(value: ModelRequirement): {
  schemaVersion: 1;
  architecture: string;
  family: string;
} {
  return { schemaVersion: 1, architecture: value.architecture, family: value.family };
}

/** Reject ASCII controls while retaining Unicode identity labels unchanged. */
function containsControlCharacter(value: string): boolean {
  for (let index = 0; index < value.length; index++) if (value.charCodeAt(index) < 32) return true;
  return false;
}
