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
/** Activation outputs built at acceptance independently preserve native delimiter and metadata contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  readExactSource,
  lineEndings,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  completionOptionTransaction,
  completionAcceptanceTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { preparedTagCompletion, refreshedTagCompletion } from './tag-completion-fixture.ts';

test('pointer and keyboard build independent normalized outputs at acceptance', () => {
  const { state: initial, result } = preparedTagCompletion();
  const option = result.options[0]!;
  for (const activation of ['keyboard', 'pointer'] as const) {
    const ready = completionAcceptanceTransaction(initial, option, activation)!;
    assert.ok(ready);
    assert.equal(ready.startState, initial);
    assert.equal(ready.isUserEvent('input.complete'), true);
    assert.equal(ready.isUserEvent('input.paste'), false);
  }
  assert.equal(
    readExactSource(completionAcceptanceTransaction(initial, option, 'pointer')!.state),
    '🙂\r\nhands\rtail',
  );
  assert.equal(
    readExactSource(completionAcceptanceTransaction(initial, option, 'keyboard')!.state),
    '🙂\r\nhands, \rtail',
  );
  const refined = initial.update({
    changes: { from: 6, insert: 'd' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  }).state;
  assert.equal(completionOptionTransaction(refined, result, 0, 'pointer'), null);
  const fresh = refreshedTagCompletion(refined);
  const index = fresh.result.options.findIndex((candidate) => candidate.label === 'hands');
  const accepted = completionOptionTransaction(fresh.state, fresh.result, index, 'pointer')!;
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state), '🙂\r\nhands\rtail');
  assert.equal(accepted.state.selection.main.head, 8);
  assert.deepEqual(accepted.state.field(lineEndings), initial.field(lineEndings));
});
