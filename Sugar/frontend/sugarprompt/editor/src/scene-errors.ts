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
/** Source-bound host scene errors are captured outside native input and retired with their exact source. */
import { StateEffect, StateField } from '@codemirror/state';
import { normalizeSceneTitle } from '../../core/src/index.js';
import { changesExactSource } from './exact-source.js';
import { editorFreshness } from './freshness.js';

/** Captured keys are editor-derived host metadata, never a mutable workflow or source owner. */
export interface SceneErrors {
  readonly sourceIdentity: symbol | null;
  readonly keys: ReadonlySet<string>;
}

const emptyErrors: SceneErrors = Object.freeze({
  sourceIdentity: null,
  keys: new Set<string>(),
});

/** Validate canonical keys and copy their mutable external container before publication. */
export function prepareSceneErrors(sourceIdentity: symbol, keys: readonly string[]): SceneErrors {
  if (
    !Array.isArray(keys) ||
    Array.from(keys).some(
      (key: unknown) => typeof key !== 'string' || !key || normalizeSceneTitle(key) !== key,
    )
  )
    throw new TypeError('Scene errors require normalized nonempty keys');
  return Object.freeze({ sourceIdentity, keys: new Set(keys) });
}

/** Publish prepared host errors only for the current exact source. */
export const installSceneErrors = StateEffect.define<SceneErrors>();

/** Native source/history changes retire external errors; local duplicate classification is core-owned. */
export const sceneErrors = StateField.define<SceneErrors>({
  /** A fresh baseline has no external validation result. */
  create() {
    return emptyErrors;
  },
  /** Stale effects cannot republish errors after typing, undo or baseline replacement. */
  update(current, transaction) {
    let next = changesExactSource(transaction) ? emptyErrors : current;
    for (const effect of transaction.effects)
      if (
        effect.is(installSceneErrors) &&
        effect.value.sourceIdentity === transaction.state.field(editorFreshness).source
      )
        next = effect.value;
    return next;
  },
});
