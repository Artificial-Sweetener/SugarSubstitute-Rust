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
/** Wildcard catalog rows replace an actual decoded placeholder through the canonical native edit producer. */
import type { EditorState } from '@codemirror/state';
import {
  isWildcardCompletionBody,
  wildcardReplacement,
  wildcardGhostText,
  type WildcardQuery,
} from '../../core/src/index.js';
import type { WildcardCatalog } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import { completionPosition, prepareScopedCompletionEdit } from './completion-splice.js';
import type {
  PreparedCompletion,
  PreparedCompletionResult,
  PreparedCompletionScope,
} from './prompt-completion-model.js';

/** Capture bounded rows outside input; storage and malformed query endpoints publish no suggestions. */
export function wildcardCompletionOptions(
  state: EditorState,
  scope: PreparedCompletionScope,
  query: WildcardQuery,
  caret: number,
  catalog: WildcardCatalog,
): PreparedCompletionResult | null {
  if (query.prefix.length > 4096) return null;
  const result = catalog.search(query.prefix);
  if (result.status !== 'ready') return null;
  const source = scope.target.scope.mapping.text,
    from = completionPosition(scope, query.replacementRange.from),
    contentFrom = completionPosition(scope, query.contentRange.from),
    to = completionPosition(scope, query.contentFieldRange.to);
  if (from === null || contentFrom === null || to === null) return null;
  const options: PreparedCompletion[] = [];
  for (const row of result.suggestions) {
    const style = row.sourceLabel === 'TXT wildcard' ? 'simple' : 'csv';
    if (!isWildcardCompletionBody(row.tag, style)) continue;
    const replacement = wildcardReplacement(source, query, row.tag, style);
    options.push(
      Object.freeze({
        kind: 'wildcard',
        label: row.tag,
        detail: row.sourceLabel,
        freshness: state.field(editorFreshness),
        caret,
        ghost: wildcardGhostText(source, query, row.tag),
        prepareEdit: (current: EditorState) =>
          prepareScopedCompletionEdit(current, scope, replacement.range, replacement.text),
      }),
    );
  }
  return options.length
    ? Object.freeze({
        from,
        to: caret,
        query: Object.freeze({
          scope: scope.target.scope,
          kind: 'wildcard',
          from,
          contentFrom,
          to,
        }),
        options: Object.freeze(options),
      })
    : null;
}
