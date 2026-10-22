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
/** Actual decoded LoRA queries produce canonical encoded native completion alternatives. */
import type { EditorState } from '@codemirror/state';
import { loraCompletionSuffix, loraReplacement, type LoraQuery } from '../../core/src/index.js';
import type { LoraCompletionCatalog } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import type {
  PreparedCompletion,
  PreparedCompletionResult,
  PreparedCompletionScope,
} from './prompt-completion-model.js';
import { stableCompletionContent } from '../../core/src/completion/continuation.js';
import { completionPosition, prepareScopedCompletionEdit } from './completion-splice.js';

/** Retain ranked canonical model identities; every decoded endpoint must have a genuine native map. */
export function loraCompletionOptions(
  state: EditorState,
  scope: PreparedCompletionScope,
  query: LoraQuery,
  caret: number,
  catalog: LoraCompletionCatalog,
): PreparedCompletionResult | null {
  const source = scope.target.scope.mapping.text,
    rows = catalog.search(query.queryText),
    from = completionPosition(scope, query.replacementRange.from),
    contentFrom = completionPosition(scope, query.nameRange.from),
    to = completionPosition(scope, query.nameFieldRange.to);
  if (!rows.length || from === null || contentFrom === null || to === null) return null;
  const stablePrefix = stableCompletionContent('lora', query.queryText),
    options: PreparedCompletion[] = [];
  for (const row of rows) {
    const replacement = loraReplacement(source, query, row.scheduleName),
      acceptance = prepareScopedCompletionEdit(
        state,
        scope,
        replacement.range,
        replacement.text,
        stablePrefix ? query.replacementRange : null,
      );
    if (!acceptance) continue;
    options.push(
      Object.freeze({
        label: row.display,
        detail: row.scheduleName,
        kind: 'lora',
        item: row.item,
        freshness: state.field(editorFreshness),
        caret,
        ghost: loraCompletionSuffix(query.queryText, row.display, row.scheduleName),
        acceptance: Object.freeze({
          keyboard: acceptance,
          pointer: acceptance,
        }),
      }),
    );
  }
  return options.length
    ? Object.freeze({
        from,
        to: caret,
        query: Object.freeze({
          scope: scope.target.scope,
          kind: 'lora',
          from,
          contentFrom,
          to,
        }),
        options: Object.freeze(options),
      })
    : null;
}
