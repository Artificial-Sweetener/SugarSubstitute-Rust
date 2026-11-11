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
/** Native state owns monotonic source and command-context epochs, without retaining a second document. */
import { StateEffect, StateField } from '@codemirror/state';
import { changesExactSource } from './exact-source.js';
import { featureProfile } from './features.js';
import { wheelMode } from './wheel-mode.js';
import { documentKind } from './document-kind.js';
import { conditioningContext } from './conditioning-context.js';

/** Retire prepared work when the host replaces an immutable semantic catalog. */
export const invalidateCommandContext = StateEffect.define<null>();

/** Process-local identities distinguish edits restored by undo and contexts restored by reconfiguration. */
export interface EditorFreshness {
  readonly source: symbol;
  readonly context: symbol;
}
/** One reason owner classifies captured source/context retirement for exact and regional host commands. */
export function freshnessRejection(
  current: EditorFreshness,
  captured: EditorFreshness,
): 'stale-source' | 'stale-context' | null {
  return current.source !== captured.source
    ? 'stale-source'
    : current.context !== captured.context
      ? 'stale-context'
      : null;
}

/** Selection, focus and derived publication reuse epochs; baseline recreation always starts a new source identity. */
export const editorFreshness = StateField.define<EditorFreshness>({
  /** A baseline is a new source/session even when its bytes equal a previous baseline. */
  create() {
    return Object.freeze({ source: Symbol(), context: Symbol() });
  },
  /** Exact newline changes and capability/catalog/permission replacement retire the appropriate prepared identity. */
  update(current, transaction) {
    const sourceChanged = changesExactSource(transaction);
    const contextChanged =
      transaction.startState.facet(conditioningContext) !==
        transaction.state.facet(conditioningContext) ||
      transaction.startState.facet(documentKind) !== transaction.state.facet(documentKind) ||
      transaction.startState.readOnly !== transaction.state.readOnly ||
      transaction.startState.facet(featureProfile) !== transaction.state.facet(featureProfile) ||
      transaction.startState.facet(wheelMode) !== transaction.state.facet(wheelMode) ||
      transaction.effects.some((effect) => effect.is(invalidateCommandContext));
    if (!sourceChanged && !contextChanged) return current;
    return Object.freeze({
      source: sourceChanged ? Symbol() : current.source,
      context: contextChanged ? Symbol() : current.context,
    });
  },
});
