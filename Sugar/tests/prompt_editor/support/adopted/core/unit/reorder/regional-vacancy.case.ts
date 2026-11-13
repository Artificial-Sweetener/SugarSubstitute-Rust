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
/** Partition-local vacancy contracts use actual preparation and immutable structural carriers. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import {
  prepareValueReorder,
  hideReorderChip,
  dropReorderChip,
  valueReorderVacancy,
  valueReorderLayout,
  serializeValueReorder,
} from '../../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Prepare the real Core operands, selecting a genuine scene scope when requested. */
function operands(source: string, held: number, scene = 0) {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
    style: 'curly',
  });
  const prepared = prepareValueReorder(scopes, scopes.scopes[scene]!)!;
  assert.ok(prepared);
  const hidden = hideReorderChip(prepared, prepared.initial, held)!;
  assert.ok(hidden);
  return { prepared, hidden };
}

for (const ending of ['\n', '\r', '\r\n'])
  test(`blank immediately before a regional fence belongs to the preceding populated partition, ending=${ending.length}`, () => {
    const { prepared, hidden } = operands(`🦊${ending}b,${ending}[SEP|Face]${ending}c`, 1);
    const layout = valueReorderLayout(hidden),
      origin = valueReorderVacancy(prepared.initial, hidden, 1);
    assert.ok(origin && origin.kind === 'gap');
    const gap = layout.gaps[origin.gap]!;
    assert.equal(gap.partition, 0);
    assert.equal(gap.carrier, 'slot');
    assert.equal(gap.blankOffsets[origin.blank], ending.length);
    assert.deepEqual(
      layout.rows.map((r) => r.partition),
      [0, 1],
    );
    const moved = dropReorderChip(prepared, hidden, 1, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!;
    assert.ok(moved);
    assert.deepEqual(moved.order, [1, 0, 2]);
    assert.deepEqual(moved.partitions, prepared.initial.partitions);
    assert.equal(
      serializeValueReorder(prepared, moved)!.text,
      `b, 🦊${ending}${ending}[SEP|Face]${ending}c`,
    );
  });

test('blank immediately after a regional fence belongs to its following populated partition', () => {
  const { prepared, hidden } = operands('a\n[SEP|Face]\nb,\nc', 1);
  const origin = valueReorderVacancy(prepared.initial, hidden, 1);
  assert.ok(origin && origin.kind === 'gap');
  const gap = valueReorderLayout(hidden).gaps[origin.gap]!;
  assert.equal(gap.partition, 1);
  assert.deepEqual(gap.blankOffsets, [12]);
  assert.equal(gap.blankOffsets[origin.blank], prepared.initial.slots[0]!.text.length);
});

test('blank lines between neighboring fences never become endpoint-partition gaps', () => {
  const { prepared, hidden } = operands('a\nb,\n[SEP|Face]\n\n[SEP|Body]\nc', 1);
  const gaps = valueReorderLayout(hidden).gaps;
  assert.equal(gaps.length, 1);
  assert.equal(gaps[0]!.partition, 0);
  assert.deepEqual(gaps[0]!.blankOffsets, [1]);
  assert.ok(valueReorderVacancy(prepared.initial, hidden, 1));
  const moved = dropReorderChip(prepared, hidden, 1, {
    kind: 'row',
    row: 0,
    insertion: 0,
  })!;
  assert.ok(moved);
  assert.equal(serializeValueReorder(prepared, moved)!.text, 'b, a\n\n[SEP|Face]\n\n[SEP|Body]\nc');
});

for (const [source, held, carrier, partition, offset] of [
  ['a\nb,\n[SEP|Face]\n', 1, 'suffix', 0, 1],
  ['[SEP|Face]\nb,\nc', 0, 'prefix', 1, 11],
] as const)
  test(`structural ${carrier} retains the genuine endpoint vacancy in its populated partition`, () => {
    const { prepared, hidden } = operands(source, held),
      origin = valueReorderVacancy(prepared.initial, hidden, held);
    assert.ok(origin && origin.kind === 'gap');
    const gap = valueReorderLayout(hidden).gaps[origin.gap]!;
    assert.equal(gap.carrier, carrier);
    assert.equal(gap.partition, partition);
    assert.equal(gap.blankOffsets[origin.blank], offset);
  });

test('nonseparator structural lines and their interior never acquire blank destinations', () => {
  const { hidden } = operands('a\nb,\n[SEP|Face]\nc', 1);
  const gap = valueReorderLayout(hidden).gaps[0]!;
  assert.deepEqual(gap.blankOffsets, [1]);
  assert.equal(gap.text.slice(gap.blankOffsets[0]), '\n[SEP|Face]\n');
});

test('a local regional move conserves a neighboring scene heading and its body', () => {
  const { prepared, hidden } = operands('a\nb,\n[SEP|Face]\nc\n**Next\nx\ny', 1);
  const scenes = prepared.scope.sceneContexts;
  assert.ok(scenes);
  assert.equal(scenes.scenes.length, 1);
  assert.equal(scenes.scenes[0]!.scene!.marker.title, 'Next');
  assert.equal(prepared.chips.length, 6);
  assert.deepEqual(prepared.initial.partitions, [0, 0, 1, 1, 1, 1]);
  assert.ok(valueReorderVacancy(prepared.initial, hidden, 1));
  const moved = dropReorderChip(prepared, hidden, 1, {
    kind: 'row',
    row: 0,
    insertion: 0,
  })!;
  assert.ok(moved);
  assert.equal(serializeValueReorder(prepared, moved)!.text, 'b, a\n\n[SEP|Face]\nc\n**Next\nx\ny');
});
