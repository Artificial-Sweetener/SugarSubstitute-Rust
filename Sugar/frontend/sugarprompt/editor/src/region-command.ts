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
/** Current canonical region identity delegates decoded name edits and exact native history to the genuine producer. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, type EditorState } from '@codemirror/state';
import { regionNameReplacement, validRegionName } from '../../core/src/index.js';
import {
  semanticStructures,
  prepareNativeStructure,
  installSemanticStructure,
} from './structure.js';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import type { RegionTarget } from './region-index.js';
import { nativeDocumentValueAt, type NativeDocumentValueContext } from './document-values.js';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from './document-value-edit-issuance.js';
import { prepareProjection } from './projection-preparation.js';
import { installProjection } from './rich-projection.js';
/** Captured actual scope-local target and current native source/context epoch. */
export interface PreparedRegionName {
  readonly target: RegionTarget;
  readonly freshness: EditorFreshness;
}
/** Actual target membership combines scope identity and local ordinal without refreshing captured intent. */
export function currentRegionName(state: EditorState, prepared: PreparedRegionName): boolean {
  const native = state.field(semanticStructures).native,
    target = prepared.target;
  return (
    !state.readOnly &&
    state.facet(richRendering) &&
    state.field(editorFreshness) === prepared.freshness &&
    !!native &&
    native.regions.find(
      (member) => member.scopeId === target.scopeId && member.ordinal === target.ordinal,
    ) === target &&
    native.nativeValues.byId[target.scopeId]?.scope?.document.regions.separators[target.ordinal] ===
      target.separator
  );
}
/** One scope-local decoded name replacement retains literal storage, current seeds, caret and native history. */
export function regionNameTransactions(
  state: EditorState,
  prepared: PreparedRegionName,
  name: string,
): readonly Transaction[] {
  if (!currentRegionName(state, prepared) || !validRegionName(name)) return [];
  const native = requireValue(state.field(semanticStructures).native, 'region-command'),
    target = prepared.target;
  const context: NativeDocumentValueContext = {
    values: native.nativeValues,
    semantics: native.documentSemantics,
    enabled: true,
    composing: false,
  };
  const admitted = nativeDocumentValueAt(state, context, {
    from: target.from,
    to: target.to,
  });
  if (admitted.status !== 'ready' || admitted.target.scope.id !== target.scopeId) return [];
  const replacement = regionNameReplacement(admitted.target.scope.document, target.ordinal, name),
    caret = replacement.from + replacement.text.length;
  const outcome = prepareDocumentValueEdit(state, {
    publication: { purpose: 'region' },
    context,
    target: admitted.target,
    range: { from: replacement.from, to: replacement.to },
    text: replacement.text,
    selection: {
      ranges: [{ kind: 'decoded', anchor: caret, head: caret }],
      mainIndex: 0,
    },
  });
  if (
    outcome.status === 'rejected' ||
    !currentRegionName(state, prepared) ||
    currentDocumentValueEdit(state, context, outcome.edit) !== null
  )
    return [];
  const edit = outcome.edit,
    post = edit.transactions.at(-1)?.state ?? state;
  const semantic = prepareNativeStructure(post, edit),
    projection = prepareProjection(post, semantic);
  if (
    !currentRegionName(state, prepared) ||
    currentDocumentValueEdit(state, context, edit) !== null
  )
    return [];
  const rendered = post.update({
    selection: post.selection,
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(projection),
    ],
    ...(outcome.status === 'unchanged' ? { annotations: Transaction.addToHistory.of(false) } : {}),
    userEvent: 'select.region',
  });
  return [...edit.transactions, rendered];
}
