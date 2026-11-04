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
/** Scene completion consumes only the actual prompt scope and literal workflow title indexes. */
import type { EditorState } from '@codemirror/state';
import { completionSuffix, sceneReplacement, type SceneQuery } from '../../core/src/index.js';
import type { SceneTitleCatalog } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import { completionPosition, prepareScopedCompletionEdit } from './completion-splice.js';
import type {
  PreparedCompletion,
  PreparedCompletionResult,
  PreparedCompletionScope,
} from './prompt-completion-model.js';

/** Prompt-only title replacements retain their exact literal source and activation policy. */
export function sceneCompletionOptions(
  state: EditorState,
  scope: PreparedCompletionScope,
  query: SceneQuery,
  caret: number,
  catalog: SceneTitleCatalog,
): PreparedCompletionResult | null {
  if (scope.context.values.scopes.values.kind !== 'prompt' || !scope.target.scope.sceneQueries)
    return null;
  const source = scope.target.scope.mapping.text,
    titles = catalog.search(query.prefix),
    from = completionPosition(scope, query.replacementRange.from);
  if (!titles.length || from === null) return null;
  const options: PreparedCompletion[] = [];
  for (const title of titles) {
    const replacement = sceneReplacement(source, query, title);
    options.push(
      Object.freeze({
        kind: 'scene',
        label: title,
        detail: 'Scene',
        freshness: state.field(editorFreshness),
        caret,
        ghost: completionSuffix(title, query.prefix),
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
          kind: 'scene',
          from,
          contentFrom: from,
          to: caret,
        }),
        options: Object.freeze(options),
      })
    : null;
}
