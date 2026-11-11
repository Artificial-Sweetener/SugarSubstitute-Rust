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
/** Exact public native selection observations are shared by finite command and continuation owners. */
import { requireValue } from '../../core/src/required.js';
import type { EditorSelection, SelectionRange } from '@codemirror/state';
/** Finite native observation captures exact public native values without JSON coercion or a selection owner. */
export interface NativeSelectionContents {
  readonly mainIndex: number;
  readonly ranges: readonly Pick<
    SelectionRange,
    'anchor' | 'head' | 'assoc' | 'bidiLevel' | 'goalColumn' | 'undirectional'
  >[];
}
/** Capture directional and vertical-navigation intent as its native typed values. */
export function selectionContents(selection: EditorSelection): NativeSelectionContents {
  return Object.freeze({
    mainIndex: selection.mainIndex,
    ranges: Object.freeze(
      selection.ranges.map((range) =>
        Object.freeze({
          anchor: range.anchor,
          head: range.head,
          assoc: range.assoc,
          bidiLevel: range.bidiLevel,
          goalColumn: range.goalColumn,
          undirectional: range.undirectional,
        }),
      ),
    ),
  });
}
/** Compare every captured range slot and public value exactly, including undefined, null and nonfinite numbers. */
export function selectionMatches(
  selection: EditorSelection,
  captured: NativeSelectionContents,
): boolean {
  if (
    !Object.is(selection.mainIndex, captured.mainIndex) ||
    selection.ranges.length !== captured.ranges.length
  )
    return false;
  for (let index = 0; index < captured.ranges.length; index++) {
    const range = selection.ranges[index],
      expected = requireValue(captured.ranges[index], 'native-selection');
    if (
      !Object.hasOwn(selection.ranges, index) ||
      !range ||
      !Object.is(range.anchor, expected.anchor) ||
      !Object.is(range.head, expected.head) ||
      !Object.is(range.assoc, expected.assoc) ||
      !Object.is(range.bidiLevel, expected.bidiLevel) ||
      !Object.is(range.goalColumn, expected.goalColumn) ||
      !Object.is(range.undirectional, expected.undirectional)
    )
      return false;
  }
  return true;
}
