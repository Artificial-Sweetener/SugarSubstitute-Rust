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
/** Ordinary reveal characterization uses real projection/history state without mounting a mouse owner or browser. */
import type { TransactionSpec } from '@codemirror/state';
import { projectionState } from './projection-state.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';

/** Retain the existing matrix's fixture contract while delegating every state/source/history change to CodeMirror. */
export function tokenPointerFixture(source: string) {
  let state = projectionState(source);
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  return {
    get state() {
      return state;
    },
    /** Apply real native transactions; the fixture does not reproduce reveal rules. */
    dispatch(spec: TransactionSpec): void {
      state = state.update(spec).state;
    },
  };
}
