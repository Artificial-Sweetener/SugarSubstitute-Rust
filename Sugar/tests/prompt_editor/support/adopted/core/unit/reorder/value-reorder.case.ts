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
/** Independent exact-source contracts exercise canonical boundary, immutable output and actual selection mapping. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { prepareValueReorder } from '../../../../../../../frontend/sugarprompt/core/src/reorder/derivation.ts';
import {
  serializeReorderPreview,
  serializeValueReorder,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/serialization.ts';
import {
  hideReorderChip,
  dropReorderChip,
  moveReorderChip,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/movement.ts';
import {
  emptyValueReorderPartitions,
  valueReorderLayout,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/layout.ts';
import {
  captureReorderSelection,
  mapReorderSelection,
} from '../../../../../../../frontend/sugarprompt/core/src/reorder/selection.ts';
import { parsePrompt } from '../../../../../../../frontend/sugarprompt/core/src/prompt/parser.ts';
import type { PreparedValueReorder } from '../../../../../../../frontend/sugarprompt/core/src/reorder/contracts.ts';

/** Prepare the actual canonical prompt scope once, with no fixture-owned grammar or authority. */
function prepare(source: string): PreparedValueReorder {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
    style: 'curly',
  });
  const result = prepareValueReorder(scopes, scopes.scopes[0]!);
  assert.ok(result);
  return result;
}

test('genuine populated preparation reuses every actual output; foreign scope and malformed order refuse', () => {
  const scopes = prepareDocumentPromptScopes(
    prepareDocumentValues('alpha, 🦊, <lora:a:1>', 'prompt'),
    { style: 'curly' },
  );
  const scope = scopes.scopes[0]!;
  const prepared = prepareValueReorder(scopes, scope)!;
  assert.equal(prepared.chips.length, 3);
  assert.equal(prepareValueReorder(scopes, scope, prepared), prepared);
  assert.equal(prepareValueReorder(scopes, scope, prepared)!.chips, prepared.chips);
  assert.equal(prepareValueReorder(scopes, { ...scope }), null);
  assert.equal(Object.isFrozen(prepared.initial.slots), true);
  assert.equal(serializeValueReorder(prepared, { ...prepared.initial, order: [0, 0, 2] }), null);
  const sparse = [...prepared.initial.order];
  // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
  delete sparse[0];
  assert.equal(serializeValueReorder(prepared, { ...prepared.initial, order: sparse }), null);
  assert.equal(hideReorderChip(prepared, prepared.initial, 99), null);
  assert.equal(
    dropReorderChip(prepared, prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 0,
    }),
    null,
  );
});

test('changed order preserves exact final comma padding and independently authored equal-weight shells', () => {
  const prepared = prepare('(a,b:1), (c,d:1),\t\r\n');
  const moved = moveReorderChip(prepared, prepared.initial, 2, {
    kind: 'row',
    row: 0,
    insertion: 0,
  })!;
  const output = serializeValueReorder(prepared, moved)!;
  assert.equal(output.text, '(c:1),(a,b:1), (d:1),\t\r\n');
  assert.equal(parsePrompt(output.text).emphasisSpans.length, 3);
  assert.deepEqual(moved.order, [2, 0, 1, 3]);
  const noOp = moveReorderChip(prepared, prepared.initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 1,
  });
  assert.equal(noOp, prepared.initial);
  assert.equal(serializeValueReorder(prepared, noOp!)!.text, prepared.scope.document.sourceText);
});

test('quoted, literal and wildcard multiline contents remain protected whole chips during genuine movement', () => {
  for (const protectedText of ['"first\r\nsecond"', '\\(first\rsecond\\)', '{first\nsecond}']) {
    const prepared = prepare(protectedText + ', tail');
    assert.deepEqual(
      prepared.chips.map((chip) => chip.text),
      [protectedText, 'tail'],
    );
    const moved = moveReorderChip(prepared, prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    const output = serializeValueReorder(prepared, moved)!;
    assert.equal(output.text, 'tail, ' + protectedText);
    assert.equal(parsePrompt(output.text).segments.length, 2);
  }
});

test('moving the only content out of a region preserves every original header and exact ending carrier', () => {
  const source = 'global\r\n[SEP|A]\rbody\n[SEP|B]\r\nlast\r';
  const prepared = prepare(source);
  const moved = moveReorderChip(prepared, prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  })!;
  const output = serializeValueReorder(prepared, moved)!;
  assert.equal(output.text, '\r\n[SEP|A]\rbody, global\n[SEP|B]\r\nlast\r');
  assert.equal(parsePrompt(output.text).regions.partitions.length, 3);
  assert.equal(parsePrompt(output.text).regions.partitions[0]!.sourceRange.to, 2);
  assert.deepEqual(output.text.match(/\r\n|\r|\n/gu), source.match(/\r\n|\r|\n/gu));
  assert.equal(Object.isFrozen(moved), true);
});

test('empty leading, intermediate and trailing parsed partitions are genuine insertion targets', () => {
  const prepared = prepare('[SEP|A]\r\nbody\r\n[SEP|B]\r\n[SEP|C]\r\n');
  const hidden = hideReorderChip(prepared, prepared.initial, 0)!;
  assert.deepEqual(
    emptyValueReorderPartitions(prepared, hidden).map((target) => target.partition),
    [0, 1, 2, 3],
  );
  const expected = [
    'body\r\n[SEP|A]\r\n\r\n[SEP|B]\r\n[SEP|C]\r\n',
    '[SEP|A]\r\nbody\r\n[SEP|B]\r\n[SEP|C]\r\n',
    '[SEP|A]\r\n\r\n[SEP|B]\r\nbody\r\n[SEP|C]\r\n',
    '[SEP|A]\r\n\r\n[SEP|B]\r\n[SEP|C]\r\nbody',
  ];
  for (const partition of [0, 1, 2, 3]) {
    const state = dropReorderChip(prepared, hidden, 0, {
      kind: 'partition',
      partition,
    })!;
    assert.ok(state);
    const output = serializeValueReorder(prepared, state)!;
    assert.equal(output.text, expected[partition]);
    assert.equal(parsePrompt(output.text).regions.partitions.length, 4);
  }
});

test('logical blank-line targets and native-relative directional selection preserve non-BMP offsets', () => {
  const prepared = prepare('first\r\n\r\nsecond, 🦊tail');
  const hidden = hideReorderChip(prepared, prepared.initial, 2)!;
  const gap = valueReorderLayout(hidden).gaps[0]!;
  assert.deepEqual(gap.blankOffsets, [2]);
  const state = dropReorderChip(prepared, hidden, 2, {
    kind: 'gap',
    gap: 0,
    blank: 0,
  })!;
  const output = serializeValueReorder(prepared, state)!;
  assert.equal(output.text, 'first\r\n🦊tail,\r\nsecond');
  const original = prepared.chips[2]!.contentRange;
  const selection = captureReorderSelection(prepared, original.to, original.from + 2)!;
  assert.equal(selection.reversed, true);
  const restored = mapReorderSelection(prepared, output, selection)!;
  assert.deepEqual(restored, {
    anchor: output.contentRanges[2]!.to,
    head: output.contentRanges[2]!.from + 2,
  });
  assert.equal(captureReorderSelection(prepared, original.from + 1, original.to), null);
  assert.equal(captureReorderSelection(prepared, 6, original.to), null);
  assert.equal(mapReorderSelection(prepared, { ...output, text: 'foreign' }, selection), null);
});

test('scope-local CSV reorder never admits headers or another value and retains quoted decoded syntax', () => {
  const source = 'name,prompt\r\nrow,"🦊, (a,b:1), <lora:x:1>"\r\nother,"neighbor, value"\r\n';
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'wildcard-csv'), {
    style: 'curly',
  });
  assert.equal(scopes.scopes.length, 4);
  assert.equal(scopes.byId['csv-container:0:1'], undefined);
  const first = prepareValueReorder(scopes, scopes.byId['csv-container:1:1']!)!;
  const neighbor = prepareValueReorder(scopes, scopes.byId['csv-container:2:1']!)!;
  const moved = moveReorderChip(first, first.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 3,
  })!;
  assert.equal(serializeValueReorder(first, moved)!.text, '(a,b:1), <lora:x:1>,🦊');
  assert.equal(serializeValueReorder(neighbor)!.text, 'neighbor, value');
  assert.equal(scopes.values.source, source);
  assert.equal(first.scope, scopes.byId['csv-container:1:1']);
  assert.notEqual(first.scope.id, neighbor.scope.id);
});

test('one-hidden preview is explicit and cannot compile as complete editable output', () => {
  const prepared = prepare('first, second\r\n');
  const hidden = hideReorderChip(prepared, prepared.initial, 1)!;
  const preview = serializeReorderPreview(prepared, hidden)!;
  assert.equal(preview.text, 'first\r\n');
  assert.equal(serializeValueReorder(prepared, hidden), null);
  assert.equal(serializeReorderPreview(prepared, prepared.initial), null);
  const selection = captureReorderSelection(
    prepared,
    prepared.scope.document.sourceText.length,
    prepared.scope.document.sourceText.length,
  )!;
  assert.equal(selection.chip, 1);
  assert.equal(selection.anchorOffset, null);
  assert.equal(selection.headOffset, null);
  const output = serializeValueReorder(prepared)!;
  assert.deepEqual(mapReorderSelection(prepared, output, selection), {
    anchor: 7,
    head: 13,
  });
});
