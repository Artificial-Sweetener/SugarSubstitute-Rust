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
/** Native derived wheel readiness marks prepared range without changing source or history. */
import { StateEffect, StateField } from '@codemirror/state';
import type { WeightSubject } from './weight-target.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';

/** Source-current logical-document UTF-16 range bounds belong to measured token readiness. */
export interface WheelFeedback extends WeightSubject {
  readonly freshness: EditorFreshness;
}

/** The finite wheel policy alone publishes or retires its prepared accent. */
export const showWheelFeedback = StateEffect.define<WheelFeedback | null>();

/** Readiness owns one current target; native widgets consume it through the existing projection owner. */
interface WheelFeedbackState {
  readonly target: WheelFeedback | null;
}

/** Empty readiness shares immutable prepared output across mount and retirement. */
const emptyFeedback: WheelFeedbackState = Object.freeze({
  target: null,
});

/** Source/context/presentation replacement clears obsolete readiness before the next native paint. */
export const wheelFeedback = StateField.define<WheelFeedbackState>({
  /** Newly mounted editors have no authorized hover. */
  create() {
    return emptyFeedback;
  },
  /** Consume only current prepared ranges; native selection is unaffected. */
  update(value, transaction) {
    if (
      transaction.startState.field(editorFreshness) !== transaction.state.field(editorFreshness) ||
      transaction.startState.facet(richRendering) !== transaction.state.facet(richRendering)
    )
      value = emptyFeedback;
    for (const effect of transaction.effects) {
      if (!effect.is(showWheelFeedback)) continue;
      const ready = effect.value;
      value =
        ready &&
        ready.freshness === transaction.state.field(editorFreshness) &&
        transaction.state.facet(richRendering) &&
        ready.range.from < ready.range.to
          ? {
              target: ready,
            }
          : emptyFeedback;
    }
    return value;
  },
});
