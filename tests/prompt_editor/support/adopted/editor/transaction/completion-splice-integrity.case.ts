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
/** An observed admitted-splice mutation stays retired after restoration while independent genuine origins remain usable. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Text } from '@codemirror/state';
import { continuationFixture } from './document-value-continuation-fixture.ts';
import {
  prepareNativeCompletionContinuation,
  completionContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';

test('all six observed splice fields retire permanently without retiring an independent genuine origin', () => {
  for (const key of ['from', 'to', 'insert', 'selection', 'effects', 'edit'] as const) {
    const f = continuationFixture();
    const prepared = prepareNativeCompletionContinuation(f.state, f.scope, f.result);
    assert.equal(prepared.status, 'ready');
    if (prepared.status !== 'ready') continue;
    const splice = f.result.options[0]!.acceptance.keyboard.splice!;
    const original = splice[key];
    const changed =
      key === 'from'
        ? f.state.doc.length + 1
        : key === 'to'
          ? -1
          : key === 'insert'
            ? Text.of(['changed'])
            : key === 'selection'
              ? EditorSelection.single(0)
              : key === 'effects'
                ? []
                : { ...f.edit };
    assert.equal(
      completionContinuationTransaction(f.state, prepared.value, f.result, 0, 'keyboard').status,
      'ready',
    );
    Object.defineProperty(splice, key, { value: changed });
    assert.equal(
      completionContinuationTransaction(f.state, prepared.value, f.result, 0, 'keyboard').status,
      'rejected',
    );
    Object.defineProperty(splice, key, { value: original });
    assert.equal(
      completionContinuationTransaction(f.state, prepared.value, f.result, 0, 'keyboard').status,
      'rejected',
    );
    const genuine = continuationFixture();
    const fresh = prepareNativeCompletionContinuation(genuine.state, genuine.scope, genuine.result);
    assert.equal(fresh.status, 'ready');
    if (fresh.status !== 'ready') continue;
    const accepted = completionContinuationTransaction(
      genuine.state,
      fresh.value,
      genuine.result,
      0,
      'keyboard',
    );
    assert.equal(accepted.status, 'ready');
    if (accepted.status === 'ready') {
      assert.equal(accepted.value, genuine.edit.transactions[0]);
      assert.equal(accepted.value.effects.at(-1), genuine.effect);
    }
  }
});
