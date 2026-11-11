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
/** Compose actual immutable native tag candidates for acceptance and activation proof. */
import assert from 'node:assert/strict';
import { EditorState, Text } from '@codemirror/state';
import { history } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { prepareTagCompletionCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import {
  tagCompletionCatalog,
  type PreparedCompletionResult,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';

/** Prepare actual candidates once from baseline or authored source; caret uses logical-document UTF-16 and subsequent native edits retire semantics. */
export function preparedTagCompletion(
  source = '🙂\r\nhan\rtail',
  caret = 6,
  authored = false,
): { readonly state: EditorState; readonly result: PreparedCompletionResult } {
  const baseline = authored ? '' : source;
  let state = EditorState.create({
    doc: Text.of(encodeSource(baseline).logical.split('\n')),
    selection: { anchor: authored ? 0 : caret },
    extensions: [
      exactSourceExtension(baseline),
      authorshipExtension,
      editorFreshness,
      semanticStructures,
      completionSession,
      richRendering.of(true),
      history(),
      tagCompletionCatalog.of(
        prepareTagCompletionCatalog({
          revision: 'continuation',
          bundled: [
            { tag: 'hands', popularity: 1 },
            { tag: 'real_(version)', popularity: 1 },
            { tag: 'soft_light', popularity: 1 },
            { tag: 'soft_lighting', popularity: 1 },
          ],
          customTags: ["o'clock_time", "𐐀'βeta_time", '"soft_light"', "'soft_light'"],
          censoredTags: [],
        }),
      ),
    ],
  });
  if (authored) {
    const ingestion = enteredTextInsertion(state, 0, 0, source);
    for (const transaction of ingestion.transactions) {
      assert.equal(transaction.startState, state);
      state = transaction.state;
    }
    state = state.update({ selection: { anchor: caret } }).state;
  }
  return refreshedTagCompletion(state);
}
/** Publish fresh semantics and suggestions for the current state, as settled native input does. */
export function refreshedTagCompletion(current: EditorState): {
  readonly state: EditorState;
  readonly result: PreparedCompletionResult;
} {
  let state = current.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(current),
    }),
  }).state;
  const result = prepareCompletionOptions(state)!;
  assert.ok(result?.options.length);
  state = state.update({ effects: installCompletion.of(result) }).state;
  return { state, result };
}
