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
/** Finite genuine CM transaction/lookup boundary; no native input, network or DOM paint qualification. */
import assert from 'node:assert/strict';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { BooruPostLookup } from '../../../../../../frontend/sugarprompt/services/src/booru/links.ts';
import { structureState } from './document-structure-fixture.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  promptHistory,
  historyTransactions,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { BooruPasteImport } from '../../../../../../frontend/sugarprompt/editor/src/booru-paste-import.ts';

/** Literal validated external DTO is boundary data, not a lookup/provider implementation. */
export function booruReply(tags = 'cat blue_eyes') {
  return {
    status: 'found',
    post: {
      id: 123,
      md5: null,
      tag_string_general: tags,
      tag_string_artist: '',
      tag_string_copyright: '',
      tag_string_character: '',
      tag_string_meta: '',
    },
  };
}
/** Capture one manually settled external request while dispatching exact actual native transaction objects. */
export function booruFixture(
  source = '',
  rich = false,
  kind: 'prompt' | 'wildcard-csv' = 'prompt',
) {
  const initial = structureState(source, kind, [
    authorshipExtension,
    promptHistory,
    richRendering.of(rich),
  ]);
  let state = initial.state,
    finish: ((value: unknown) => void) | null = null,
    fail: ((cause: unknown) => void) | null = null,
    signal: AbortSignal | null = null;
  const transactions: Transaction[] = [];
  let observer: ((transaction: Transaction) => void) | null = null;
  const lookup: BooruPostLookup = (_key, abort) => {
    signal = abort;
    return new Promise((resolve, reject) => {
      finish = resolve;
      fail = reject;
    });
  };
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('booru-fixture-native-view');
  Object.defineProperties(boundary, {
    state: { get: () => state },
    composing: { value: false, writable: true },
    compositionStarted: { value: false, writable: true },
    hasFocus: { value: true, writable: true },
    dispatch: {
      value: (input: Transaction | TransactionSpec) => {
        const transaction = input instanceof Transaction ? input : state.update(input);
        assert.equal(transaction.startState, state);
        state = transaction.state;
        transactions.push(transaction);
        owner.update({ transactions: [transaction], focusChanged: false });
        observer?.(transaction);
        const restored = historyTransactions([transaction]);
        if (restored.length > 1) {
          const selection = restored[1]!;
          assert.equal(selection.startState, state);
          state = selection.state;
          transactions.push(selection);
          owner.update({ transactions: [selection], focusChanged: false });
        }
      },
    },
  });
  const owner = new BooruPasteImport(
    boundary,
    () => lookup,
    () => undefined,
  );
  return {
    view: boundary,
    owner,
    initial,
    transactions,
    state: () => state,
    signal: () => signal,
    /** Resolve the sole captured host boundary only after the real service starts it. */
    resolve(value: unknown) {
      assert.ok(finish);
      finish(value);
    },
    /** Reject with the original unexpected host cause. */
    reject(cause: unknown) {
      assert.ok(fail);
      fail(cause);
    },
    /** Observe synchronous native publication without replacing dispatch or parser ownership. */
    observe(next: (transaction: Transaction) => void) {
      observer = next;
    },
    /** Release this finite fixture's pending external/native capability. */
    destroy() {
      owner.destroy();
    },
  };
}

/** Service lookup starts in its owned promise task, without arbitrary time settling. */
export async function lookupStarted(): Promise<void> {
  await Promise.resolve();
}
