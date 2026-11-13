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
/** Source-bound scene queue publication consumes prepared host authority and the sole native semantic owner. */
import { requireValue } from '../../core/src/required.js';
import { Facet, StateEffect, StateField, type EditorState } from '@codemirror/state';
import type {
  PreparedSceneQueue,
  SceneQueueOutcome,
  SceneQueueRequest,
} from '../../services/src/scenes/queue.js';
import { sourcePositionsToDocument } from '../../core/src/line-endings.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { documentKind } from './document-kind.js';
/** Host callbacks observe source-free outcomes or current unexpected failures. */
export interface SceneQueueHost {
  readonly capability: PreparedSceneQueue;
  readonly observe: (outcome: SceneQueueOutcome) => void;
  readonly report: (cause: unknown) => void;
}
/** Native composition has one immutable capability publication. */
export const sceneQueueHost = Facet.define<SceneQueueHost | null, SceneQueueHost | null>({
  combine: (values) => values.at(-1) ?? null,
});
/** Source edits retire authority even if the unchanged host prop remains installed. */
export const sceneQueuePublication = StateField.define<SceneQueueHost | null>({
  create: (state) => (state.facet(documentKind) === 'prompt' ? state.facet(sceneQueueHost) : null),
  update(current, transaction) {
    if (
      transaction.state.facet(documentKind) !== 'prompt' ||
      changesExactSource(transaction) ||
      transaction.startState.facet(documentKind) !== transaction.state.facet(documentKind)
    )
      return null;
    return transaction.startState.facet(sceneQueueHost) !== transaction.state.facet(sceneQueueHost)
      ? transaction.state.facet(sceneQueueHost)
      : current;
  },
});
/** One accepted target binds a source-free prepared host request to native semantic context. */
export interface PreparedSceneQueueTarget {
  readonly freshness: EditorFreshness;
  readonly host: SceneQueueHost;
  readonly request: SceneQueueRequest;
}
/** Derived native lookup uses logical-document UTF-16 boundaries and reuses prepared request identities. */
export interface SceneQueueTargets {
  readonly host: SceneQueueHost;
  readonly source: symbol;
  at(position: number): SceneQueueRequest | null;
}
/** Install prepared lookup outside input; stale source/host effects cannot restore queue eligibility. */
export const installSceneQueueTargets = StateEffect.define<SceneQueueTargets>();
/** Native transactions retain only source-current derived queue hit testing. */
export const sceneQueueTargets = StateField.define<SceneQueueTargets | null>({
  create: () => null,
  update(current, transaction) {
    const host = transaction.state.field(sceneQueuePublication),
      source = transaction.state.field(editorFreshness).source;
    if (current?.host !== host || current?.source !== source) current = null;
    for (const effect of transaction.effects)
      if (
        effect.is(installSceneQueueTargets) &&
        effect.value.host === host &&
        effect.value.source === source
      )
        current = effect.value;
    return current;
  },
});
/** Convert all parsed scene ownership starts once; native pointer/keyboard hit testing performs only binary search. */
export function prepareSceneQueueTargets(state: EditorState): SceneQueueTargets | null {
  const host = state.field(sceneQueuePublication),
    semantic = state.field(semanticStructures).native;
  if (!host || semantic?.kind !== 'prompt' || !semantic.documentSemantics.scenesEnabled)
    return null;
  const contexts =
    semantic.sceneContexts.at(0) === semantic.sceneContexts.universal
      ? [semantic.sceneContexts.universal, ...semantic.sceneContexts.scenes]
      : semantic.sceneContexts.scenes;
  const starts = sourcePositionsToDocument(
    state.doc.toString(),
    state.field(lineEndings),
    contexts.map((context) => context.sourceRange.from),
  );
  const requests = contexts.map((context) =>
    context.scene ? host.capability.request(context.scene.marker.normalizedKey) : null,
  );
  const length = state.doc.length;
  return Object.freeze({
    host,
    source: semantic.sourceIdentity,
    /** Physical heading/body ownership follows the prepared core context, including the final ending caret. */
    at(position: number): SceneQueueRequest | null {
      if (!Number.isSafeInteger(position) || position < 0 || position > length)
        throw new RangeError('Invalid queue document position');
      let from = 0,
        to = starts.length;
      while (from < to) {
        const middle = (from + to) >>> 1;
        if (requireValue(starts[middle], 'scene-queue') <= position) from = middle + 1;
        else to = middle;
      }
      return requests[from - 1] ?? null;
    },
  });
}
/** Mutation visibility follows host eligibility and current native scene permission, never the caret's other scene. */
export function sceneQueueTargetAt(
  state: EditorState,
  position: number,
): PreparedSceneQueueTarget | null {
  if (state.readOnly || state.facet(documentKind) !== 'prompt') return null;
  const targets = state.field(sceneQueueTargets, false),
    request = targets?.at(position);
  return targets && request
    ? { host: targets.host, request, freshness: state.field(editorFreshness) }
    : null;
}
/** Caret-driven folding does not revoke an accepted intent; source, workflow, permission and profile changes do. */
export function currentSceneQueueTarget(
  state: EditorState,
  target: PreparedSceneQueueTarget,
): boolean {
  return (
    !state.readOnly &&
    state.facet(documentKind) === 'prompt' &&
    state.field(editorFreshness) === target.freshness &&
    state.field(sceneQueuePublication) === target.host
  );
}
