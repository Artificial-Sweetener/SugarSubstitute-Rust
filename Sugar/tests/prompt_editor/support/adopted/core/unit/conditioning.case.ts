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
/** Independent conditioning contracts distinguish regional inheritance from separator syntax and unavailable host authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ConditioningTopology,
  parsePrompt,
  type ConditioningMode,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

test('independent and unresolved partitions emit separate roots while regional children inherit only global source', () => {
  const source = 'global  \r\n[SEP|Face]\r\nred\r[SEP]\nblue';
  const document = parsePrompt(source);
  for (const mode of ['independent', 'unresolved', 'regional'] as const) {
    const topology = new ConditioningTopology(mode, document.regions);
    assert.equal(topology.scopes.length, 3);
    assert.deepEqual(
      topology.emitted.map((scope) =>
        topology.effectiveRanges(scope.id).map((range) => source.slice(range.from, range.to)),
      ),
      mode === 'regional'
        ? [
            ['global  \r\n', 'red\r'],
            ['global  \r\n', 'blue'],
          ]
        : [['global  \r\n'], ['red\r'], ['blue']],
    );
    assert.equal(topology.roots.length, mode === 'regional' ? 1 : 3);
    assert.equal(topology.partition(1), topology.scopes[1]);
    assert.equal(topology.at(source.indexOf('red')), topology.scopes[1]);
    assert.equal(topology.at(source.indexOf('[SEP')), null);
    assert.equal(topology.at(source.length), null);
    assert.equal(topology.at(-1), null);
    assert.equal(topology.effectiveRanges('partition:1'), topology.effectiveRanges('partition:1'));
    assert.equal(document.sourceText === source, true);
    assert.throws(() => topology.partition(8), RangeError);
  }
});

test('empty partitions remain emitted children and separator-free source emits one condition in every context', () => {
  const empty = new ConditioningTopology('regional', parsePrompt('[SEP]\r\n[SEP]\n').regions);
  assert.deepEqual(
    empty.scopes.map((scope) => [scope.role, scope.emits, scope.range.from === scope.range.to]),
    [
      ['global', false, true],
      ['region', true, true],
      ['region', true, true],
    ],
  );
  assert.deepEqual(empty.children('partition:0'), empty.emitted);
  assert.equal(empty.at(0), null);
  for (const mode of ['independent', 'unresolved', 'regional'] satisfies ConditioningMode[]) {
    const topology = new ConditioningTopology(mode, parsePrompt('').regions);
    assert.deepEqual(
      topology.scopes.map((scope) => [scope.role, scope.parent, scope.emits]),
      [['condition', null, true]],
    );
    assert.equal(topology.at(0), null);
    assert.deepEqual(topology.children('partition:0'), []);
  }
});
