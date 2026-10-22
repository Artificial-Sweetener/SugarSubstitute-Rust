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
/** Native hit testing consumes canonical value coordinates and actual prepared schedule entries. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import type { PreparedDocumentLoraSchedules } from '../../services/src/lora/scene-schedules.js';
import type { PreparedNativeDocumentValues } from './document-values.js';
/** Prepare a logical UTF-16 lookup; protected storage, encoding gaps and trimmed padding never acquire trigger authority. */
export function prepareLoraTriggerEligibility(
  state: EditorState,
  values: PreparedNativeDocumentValues,
  schedules: PreparedDocumentLoraSchedules,
): (position: number, backendKey?: string) => boolean {
  const entries = values.entries,
    length = state.doc.length;
  /** Binary storage lookup maps only a canonical safe endpoint into the existing schedule owner. */
  return (position: number, backendKey?: string): boolean => {
    if (!Number.isSafeInteger(position) || position < 0 || position > length)
      throw new RangeError('Invalid trigger document position');
    let from = 0,
      to = entries.length;
    while (from < to) {
      const middle = (from + to) >>> 1;
      if (requireValue(entries[middle], 'lora-trigger-eligibility').logicalStorage.to < position)
        from = middle + 1;
      else to = middle;
    }
    const entry = entries[from],
      map = entry?.semanticMapping;
    if (!entry?.scope || !map || entry.logicalStorage.from > position) return false;
    const decoded = map.logicalToDecoded[position - map.logicalRange.from];
    if (decoded == null) return false;
    const exact = map.mapping.decodedToSource[decoded];
    return exact != null && schedules.canInsertAt(exact, backendKey);
  };
}
