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
/** Shared selection/session proof uses actual canonical scopes and genuinely issued command operands. */
import type { EditorState, Extension } from '@codemirror/state';
import { structureState, publishStructure } from './document-structure-fixture.ts';
import {
  prepareNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { completionSession } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  prepareCompletionScope,
  prepareScopedCompletionEdit,
  completionPosition,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import type {
  PreparedCompletionScope,
  PreparedModelCompletions,
  PreparedWordCompletions,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import type { LoraCatalogItem } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { loraReplacement } from '../../../../../../frontend/sugarprompt/core/src/completion/lora-text.ts';

/** Independent row identities preserve the deliberately ambiguous model-file specimens. */
export interface SessionRow {
  readonly name: string;
  readonly backend: string;
}
/** Native source/caret/structure authority is installed once before independent candidate preparation. */
export function completionState(
  source = 'ha',
  caret = source.length,
  extra: Extension = [],
): EditorState {
  const baseline = structureState(source, 'prompt', [completionSession, extra]).state;
  const state = baseline.update({ selection: { anchor: caret } }).state;
  return refreshCompletionState(state);
}
/** Refresh only through the actual cold preparation/publication owner after a genuine native source transition. */
export function refreshCompletionState(state: EditorState): EditorState {
  return publishStructure(state, prepareNativeStructure(state));
}
const defaultRows: readonly SessionRow[] = [
  { name: 'hair', backend: 'hair.pt' },
  { name: 'hands', backend: 'hands.pt' },
];
/** Literal independent tag rows obtain their own genuine native command edits at the current admitted scope. */
export function sessionResult(
  state: EditorState,
  rows: readonly SessionRow[] = defaultRows,
): PreparedWordCompletions {
  const scope = currentScope(state);
  const caret = state.selection.main.head;
  const query = scope.target.scope.tagQueries.queryAt(scope.target.decodedRange.from, false);
  if (!query) throw new Error('fixture-canonical-query-missing');
  const range = query.replacementRange;
  const from = completionPosition(scope, range.from);
  const to = completionPosition(scope, query.activeTagEnd);
  if (from === null || to === null) throw new Error('fixture-query-map-missing');
  return {
    surface: 'words',
    from,
    to: caret,
    query: { scope: scope.target.scope, kind: 'tag', from, contentFrom: from, to },
    options: rows.map((row) => ({
      kind: 'tag' as const,
      ...proposal(state, scope, row.name, { range, text: row.name }),
    })),
  };
}
/** Literal independent model rows replace the whole current `<lora:` token through genuine native edits. */
export function modelSessionResult(
  state: EditorState,
  rows: readonly SessionRow[] = defaultRows,
): PreparedModelCompletions {
  const scope = currentScope(state);
  const caret = state.selection.main.head;
  const query = scope.target.scope.loraQueries.queryAt(scope.target.decodedRange.from, false);
  if (!query) throw new Error('fixture-canonical-query-missing');
  const from = completionPosition(scope, query.replacementRange.from);
  const contentFrom = completionPosition(scope, query.nameRange.from);
  const to = completionPosition(scope, query.nameFieldRange.to);
  if (from === null || contentFrom === null || to === null)
    throw new Error('fixture-query-map-missing');
  return {
    surface: 'models',
    from,
    to: caret,
    query: { scope: scope.target.scope, kind: 'lora', from, contentFrom, to },
    options: rows.map((row) => ({
      kind: 'lora' as const,
      item: sessionItem(row),
      ...proposal(
        state,
        scope,
        row.name,
        loraReplacement(scope.target.scope.mapping.text, query, row.name),
      ),
    })),
  };
}
/** A minimal catalog item whose prompt name and installed file are the row's literal identities. */
export function sessionItem(row: SessionRow): LoraCatalogItem {
  return {
    model: { promptName: row.name, backendValue: row.backend, basename: row.name },
    title: row.name,
    subtitle: null,
    thumbnail: { status: 'unavailable' },
    triggerWords: [],
    modelPageUrl: null,
  };
}
/** The admitted caret scope must be the one the current native structure published. */
function currentScope(state: EditorState): PreparedCompletionScope {
  const scope = prepareCompletionScope(state);
  if (!scope || state.field(semanticStructures).native?.nativeValues !== scope.context.values)
    throw new Error('fixture-current-completion-scope-missing');
  return scope;
}
/** Shared display metadata; the edit is proven genuine now and issued again on acceptance. */
function proposal(
  state: EditorState,
  scope: PreparedCompletionScope,
  name: string,
  replacement: {
    readonly range: { readonly from: number; readonly to: number };
    readonly text: string;
  },
) {
  if (!prepareScopedCompletionEdit(state, scope, replacement.range, replacement.text))
    throw new Error('fixture-genuine-completion-edit-rejected');
  return {
    label: name,
    detail: '',
    freshness: state.field(editorFreshness),
    caret: state.selection.main.head,
    ghost: name.slice(2),
    prepareEdit: (current: EditorState) =>
      prepareScopedCompletionEdit(current, scope, replacement.range, replacement.text),
  };
}
