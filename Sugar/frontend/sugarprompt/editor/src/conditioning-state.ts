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
/** Source-current prepared conditioning belongs to one derived state field, independently of parser grammar and presentation. */
import { StateEffect, StateField, type EditorState } from '@codemirror/state';
import { ConditioningTopology } from '../../core/src/index.js';
import type { ConditioningContext } from '../../services/src/index.js';
import { conditioningContext } from './conditioning-context.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { documentKind } from './document-kind.js';
import type { PreparedStructure } from './structure.js';

/** Context and source identity bind exact-source UTF-16 topology to its one current endpoint. */
export interface PreparedConditioning {
  readonly freshness: EditorFreshness;
  readonly context: ConditioningContext;
  readonly topology: ConditioningTopology;
}
/** Only already prepared topology may enter native transaction/paint paths. */
export const installConditioning = StateEffect.define<PreparedConditioning>();
/** Source, endpoint topology and permissions retire a captured host observation before later publication. */
export const nativeConditioning = StateField.define<PreparedConditioning | null>({
  /** Initial preparation follows semantic settlement or explicit host inspection. */
  create: () => null,
  /** Bounded identity checks accept current prepared values without parsing or traversing partitions. */
  update(current, transaction) {
    /** Admission consumes the existing source/context and document-kind owners. */
    const valid = (value: PreparedConditioning) =>
      value.freshness === transaction.state.field(editorFreshness) &&
      value.context === transaction.state.facet(conditioningContext) &&
      transaction.state.facet(documentKind) === 'prompt';
    if (current && !valid(current)) current = null;
    for (const effect of transaction.effects)
      if (effect.is(installConditioning) && valid(effect.value)) current = effect.value;
    return current;
  },
});
/** Build prompt topology outside native input/measurement; candidate storage is explicitly unsupported. */
export function prepareConditioning(
  state: EditorState,
  semantic: PreparedStructure,
): PreparedConditioning {
  if (semantic.kind !== 'prompt') throw new Error('conditioning-requires-prompt');
  const current = state.field(nativeConditioning),
    freshness = state.field(editorFreshness),
    context = state.facet(conditioningContext);
  if (current?.freshness === freshness && current.context === context) return current;
  if (semantic.sourceIdentity !== freshness.source) throw new Error('Stale conditioning source');
  return Object.freeze({
    freshness,
    context,
    topology: new ConditioningTopology(context.mode, semantic.structure.document.regions),
  });
}
