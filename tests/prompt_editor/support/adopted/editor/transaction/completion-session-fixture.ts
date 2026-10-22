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
import { completionWall } from '../../../../../../frontend/sugarprompt/editor/src/completion-wall-state.ts';
import {
  prepareCompletionScope,
  prepareScopedCompletionEdit,
  completionPosition,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import type { PreparedCompletionResult } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
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
  const baseline = structureState(source, 'prompt', [
    completionSession,
    completionWall,
    extra,
  ]).state;
  const state = baseline.update({ selection: { anchor: caret } }).state;
  return refreshCompletionState(state);
}
/** Refresh only through the actual cold preparation/publication owner after a genuine native source transition. */
export function refreshCompletionState(state: EditorState): EditorState {
  return publishStructure(state, prepareNativeStructure(state));
}
/** Literal independent candidate rows obtain their own genuine native command edits at the current admitted scope. */
export function sessionResult(
  state: EditorState,
  rows: readonly SessionRow[] = [
    { name: 'hair', backend: 'hair.pt' },
    { name: 'hands', backend: 'hands.pt' },
  ],
  kind: 'tag' | 'lora' = 'tag',
): PreparedCompletionResult {
  const scope = prepareCompletionScope(state);
  if (!scope || state.field(semanticStructures).native?.nativeValues !== scope.context.values)
    throw new Error('fixture-current-completion-scope-missing');
  const caret = state.selection.main.head;
  const decoded = scope.target.decodedRange.from;
  const query =
    kind === 'tag'
      ? scope.target.scope.tagQueries.queryAt(decoded, false)
      : scope.target.scope.loraQueries.queryAt(decoded, false);
  if (!query) throw new Error('fixture-canonical-query-missing');
  const range = query.replacementRange;
  const from = completionPosition(scope, range.from);
  const contentFrom = completionPosition(
    scope,
    'nameRange' in query ? query.nameRange.from : range.from,
  );
  const to = completionPosition(
    scope,
    'nameFieldRange' in query ? query.nameFieldRange.to : query.activeTagEnd,
  );
  if (from === null || contentFrom === null || to === null)
    throw new Error('fixture-query-map-missing');
  return {
    from,
    to: caret,
    query: { scope: scope.target.scope, kind, from, contentFrom, to },
    options: rows.map((row) => {
      const replacement =
        'nameRange' in query
          ? loraReplacement(scope.target.scope.mapping.text, query, row.name)
          : { range, text: row.name };
      const acceptance = prepareScopedCompletionEdit(
        state,
        scope,
        replacement.range,
        replacement.text,
        range,
      );
      if (!acceptance) throw new Error('fixture-genuine-completion-edit-rejected');
      return {
        label: row.name,
        detail: '',
        freshness: state.field(editorFreshness),
        caret,
        ghost: row.name.slice(2),
        acceptance: { keyboard: acceptance, pointer: acceptance },
        ...(kind === 'tag'
          ? { kind: 'tag' as const }
          : {
              kind: 'lora' as const,
              item: {
                model: {
                  promptName: row.name,
                  backendValue: row.backend,
                  basename: row.name,
                },
                title: row.name,
                subtitle: null,
                thumbnail: { status: 'unavailable' as const },
                triggerWords: [],
                modelPageUrl: null,
              },
            }),
      };
    }),
  };
}
