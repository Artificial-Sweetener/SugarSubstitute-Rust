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
/** Real catalogs and native state prove which surface a query reaches and where a model token begins and ends. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import { installCompletion } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { completionOptionTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { modelResult, modelState, FakeModelPopupHost } from './model-completion-fixture.ts';
import { CompletionDocument } from '../unit/completion-dom-fixture.ts';

/** Accept the first candidate by keyboard for `source` at `caret` and return the resulting state. */
function acceptFirst(source: string, caret?: number) {
  let state = modelState(source, caret, { host: new FakeModelPopupHost(new CompletionDocument()) });
  const result = prepareCompletionOptions(state);
  assert.equal(result?.surface, 'models');
  state = state.update({ effects: installCompletion.of(result) }).state;
  const accepted = completionOptionTransaction(state, result, 0, 'keyboard');
  assert.ok(accepted);
  return { before: state, after: accepted.state, label: result.options[0]!.label };
}

test('a LoRA name query reaches only the model surface; tag text never reaches it', () => {
  const models = modelResult('<lora:mid');
  assert.equal(models?.surface, 'models');
  assert.ok(models.options.length > 0);
  assert.ok(models.options.every((option) => option.kind === 'lora'));
  // The tag catalog also matches "mid", yet no word row is produced inside the model name.
  assert.deepEqual(
    models.options.map((option) => option.item.model.promptName),
    ['folder/midday', 'folder/midnight'],
  );
  const words = modelResult('mid');
  assert.equal(words?.surface, 'words');
  assert.ok(words.options.length > 0);
  assert.ok(words.options.every((option) => option.kind !== ('lora' as string)));
  for (const source of ['mid', 'a, mid', '<lora:x:1.00> mid', 'midnight'])
    assert.notEqual(modelResult(source)?.surface, 'models');
});

test('a LoRA query produces nothing without a host gallery, even with a catalog', () => {
  assert.equal(modelResult('<lora:mid', undefined, { host: null }), null);
  assert.equal(modelResult('<lora:', undefined, { host: null }), null);
  // Word completion is unaffected by the missing gallery.
  assert.equal(modelResult('mid', undefined, { host: null })?.surface, 'words');
});

test('the trigger is the last case-insensitive marker before the caret with no closing bracket between', () => {
  for (const [source, caret] of [
    ['<LoRA:mid', 9],
    ['<lora:x:1.00> <lora:mid', 23],
    ['<lora:mid>', 9],
    ['<lora:mid:0.5>', 9],
    ['<lora:', 6],
    ['a, <lora:mi tail', 11],
  ] as const)
    assert.equal(modelResult(source, caret)?.surface, 'models', `${source} @${caret}`);
  for (const [source, caret] of [
    ['<lora:mid>', 10],
    ['<lora:mid:0.5>', 10],
    ['<lora', 5],
    ['<lora:x> mid', 12],
  ] as const)
    assert.notEqual(modelResult(source, caret)?.surface, 'models', `${source} @${caret}`);
});

test('an empty LoRA query lists every installed model', () => {
  const rows = Array.from({ length: 40 }, (_, index) => ({ promptName: `m/model${index}` }));
  const result = modelResult('<lora:', undefined, { rows });
  assert.equal(result?.surface, 'models');
  assert.equal(result.options.length, 40);
});

test('the token ends at its closing bracket, or else at the next separator after the caret', () => {
  for (const [source, caret, accepted] of [
    ['<lora:mi', 8, '<lora:folder/midday:1.00>'],
    ['<lora:mi>', 8, '<lora:folder/midday:1.00>'],
    ['<lora:mixx> tail', 8, '<lora:folder/midday:1.00> tail'],
    ['<lora:mixx, tail', 8, '<lora:folder/midday:1.00>, tail'],
    ['<lora:mixx tail', 8, '<lora:folder/midday:1.00> tail'],
    ['<lora:mixx\ttail', 8, '<lora:folder/midday:1.00>\ttail'],
    ['<lora:mixx\ntail', 8, '<lora:folder/midday:1.00>\ntail'],
    // A bracket after another opener belongs to that later token.
    ['<lora:mi <x>', 8, '<lora:folder/midday:1.00> <x>'],
  ] as const)
    assert.equal(readExactSource(acceptFirst(source, caret).after), accepted, source);
});

test('acceptance writes the prompt name with the typed weight or 1.00, puts the caret after it and adds no comma', () => {
  for (const [source, caret, accepted] of [
    ['a, <lora:mid', 12, 'a, <lora:folder/midday:1.00>'],
    ['a, <lora:mid:0.7>', 12, 'a, <lora:folder/midday:0.7>'],
    ['a, <lora:mid:bad>', 12, 'a, <lora:folder/midday:1.00>'],
  ] as const) {
    const { before, after } = acceptFirst(source, caret);
    assert.equal(readExactSource(after), accepted);
    assert.equal(after.selection.main.head, accepted.indexOf('>') + 1);
    let state = after;
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), source);
    assert.equal(state.selection.main.head, before.selection.main.head);
  }
});
