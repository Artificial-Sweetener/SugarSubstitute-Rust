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
/** Enforce the parser and native editor's established required-value invariants at runtime. */

/** Fail at the owning source boundary instead of propagating an absent indexed value through a rich edit. */
export function requireValue<T>(value: T, owner: string): NonNullable<T> {
  if (value === undefined || value === null)
    throw new Error(`Missing required SugarPrompt value in ${owner}.`);
  return value;
}
