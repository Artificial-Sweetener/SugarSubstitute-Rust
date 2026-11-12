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
/** Source-current native numeric intent contains no draft text, DOM, geometry or competing caret. */
import { StateEffect, StateField } from '@codemirror/state';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { weightEditingAllowed, type WeightSubject } from './weight-target.js';
/** Logical-document UTF-16 identifies the original numeric subject; one key owns its finite native input. */
export interface ExactWeightDraft extends WeightSubject {
  readonly freshness: EditorFreshness;
  readonly key: symbol;
  readonly phase: 'editing' | 'awaiting-commit';
}
/** Opening/closing the native cell is projection intent, independently of committed prompt/history. */
export const setExactWeightDraft = StateEffect.define<ExactWeightDraft | null>();
/** Source/context/permissions retire intent before an obsolete widget can mount an input. */
export const exactWeightDraft = StateField.define<ExactWeightDraft | null>({
  /** Baselines contain no native numeric draft. */
  create: () => null,
  /** Only current editable rich subjects admit publication. */
  update(current, transaction) {
    /** Validate publication against the native source and permission owners. */
    const valid = (value: ExactWeightDraft): boolean =>
      (value.phase === 'editing' || value.phase === 'awaiting-commit') &&
      value.freshness === transaction.state.field(editorFreshness) &&
      transaction.state.facet(richRendering) &&
      weightEditingAllowed(transaction.state, value.kind);
    if (current && !valid(current)) current = null;
    for (const effect of transaction.effects)
      if (effect.is(setExactWeightDraft) && (!effect.value || valid(effect.value)))
        current = effect.value;
    return current;
  },
});
