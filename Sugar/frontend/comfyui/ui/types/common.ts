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
/** Define shared value contracts used across the typed frontend. */

export type Vec2 = [number, number];
export type Bounds = [number, number, number, number];
export type UnknownRecord = Record<string, unknown>;

export interface RectBounds extends UnknownRecord {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Return whether a dynamic value is a non-null object with string keys. */
export function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null;
}

/** Read a trimmed string property from a dynamic boundary object. */
export function readString(record: UnknownRecord, key: string): string {
  const value = record[key];
  return typeof value === 'string' ? value.trim() : '';
}
