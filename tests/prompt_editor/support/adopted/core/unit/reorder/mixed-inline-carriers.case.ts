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
/** Independent mixed comma/adjacent-LoRA carrier contracts conserve source-owned delimiters and provenance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepare, coverage } from './origin-map-fixture.ts';
import {
  hideReorderChip,
  moveReorderChip,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/movement.ts';
import {
  serializeReorderPreview,
  serializeValueReorder,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/serialization.ts';
import { sourceText } from '../../../../../../../frontend/sugarprompt/core/src/reorder/source-pieces.ts';
import { valueReorderLayout } from '../../../../../../../frontend/sugarprompt/core/src/reorder/layout.ts';

for (const specimen of [
  {
    name: 'established comma spacing',
    source: 'head, <lora:a:1>    <lora:b:1>, tail',
    expected: 'head, <lora:b:1>, tail',
    carrier: ', ',
  },
  {
    name: 'unspaced comma and authored tabs',
    source: 'head,<lora:a:1>\t\t<lora:b:1>, tail',
    expected: 'head,\t\t<lora:b:1>, tail',
    carrier: ',\t\t',
  },
  {
    name: 'empty adjacency',
    source: 'head,\t<lora:a:1><lora:b:1>, tail',
    expected: 'head,\t<lora:b:1>, tail',
    carrier: ',\t',
  },
  {
    name: 'CRLF carrier',
    source: 'head,\r\n<lora:a:1>\t<lora:b:1>, tail',
    expected: 'head,\r\n<lora:b:1>, tail',
    carrier: ',\r\n',
  },
  {
    name: 'regional carrier',
    source: 'head,\r\n[SEP|Face]\r\n<lora:a:1>    <lora:b:1>, tail',
    expected: 'head,\r\n[SEP|Face]\r\n<lora:b:1>, tail',
    carrier: ',\r\n[SEP|Face]\r\n',
  },
  {
    name: 'shared emphasis shell',
    source: '(head, <lora:a:1>    <lora:b:1>:2), tail',
    expected: '(head, <lora:b:1>:2), tail',
    carrier: ', ',
  },
])
  test(`hiding the genuine middle LoRA retains ${specimen.name}`, () => {
    const prepared = prepare(specimen.source),
      initial = prepared.initial,
      hidden = hideReorderChip(prepared, initial, 1);
    assert.equal(prepared.chips.length, 4);
    assert.ok(hidden);
    assert.deepEqual(hidden.order, [0, 2, 3]);
    assert.equal(hidden.slots[0]!.text === specimen.carrier, true);
    assert.equal(hidden.partitions === initial.partitions, true);
    assert.deepEqual(hidden.partitions, initial.partitions);
    const output = serializeReorderPreview(prepared, hidden);
    assert.ok(output);
    assert.equal(output.text === specimen.expected, true);
    coverage(prepared, output);
    const carrier = hidden.slots[0]!;
    assert.equal(
      carrier.pieces.every((piece) => piece.kind === 'source'),
      true,
    );
    assert.equal(
      carrier.pieces
        .map((piece) =>
          piece.kind === 'source'
            ? specimen.source.slice(piece.sourceRange.from, piece.sourceRange.to)
            : '',
        )
        .join('') === specimen.carrier,
      true,
    );
    assert.equal(Object.isFrozen(hidden), true);
    assert.equal(serializeValueReorder(prepared, initial)!.text === specimen.source, true);
  });

test('a real unchanged move retains exact mixed spacing while a genuine move uses the destination row delimiter', () => {
  const source = 'head, <lora:a:1>    <lora:b:1>, tail',
    prepared = prepare(source),
    initial = prepared.initial;
  const retained = moveReorderChip(prepared, initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 1,
  });
  assert.equal(retained === initial, true);
  const moved = moveReorderChip(prepared, initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 3,
  });
  assert.ok(moved);
  assert.deepEqual(moved.order, [0, 2, 3, 1]);
  const output = serializeValueReorder(prepared, moved);
  assert.ok(output);
  assert.equal(output.text === 'head, <lora:b:1>, tail, <lora:a:1>', true);
  assert.equal(valueReorderLayout(moved).rows.length, 1);
  coverage(prepared, output);
});

test('the narrow horizontal case cannot merge arbitrary nonseparator source bytes', () => {
  const source = 'head, <lora:a:1>    <lora:b:1>, tail',
    prepared = prepare(source),
    slots = [...prepared.initial.slots];
  slots[1] = sourceText(prepared.scope, source.indexOf('tail'), source.length);
  assert.equal(hideReorderChip(prepared, { ...prepared.initial, slots }, 1), null);
});
