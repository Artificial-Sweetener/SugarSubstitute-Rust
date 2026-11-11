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
/** Independent hidden-topology contracts locate the exact held vacancy without browser coordinates. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { prepareValueReorder } from '../../../../../../../frontend/sugarprompt/core/src/reorder/derivation.ts';
import { hideReorderChip } from '../../../../../../../frontend/sugarprompt/core/src/reorder/movement.ts';
import {
  valueReorderVacancy,
  dropReorderChip,
  serializeValueReorder,
  valueReorderLayout,
  valueReorderAdjacentTarget,
} from '../../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Actual Core preparation/hiding supplies the operands; expected topology is authored independently. */
function operands(source: string, held: number) {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
      style: 'curly',
    }),
    prepared = prepareValueReorder(scopes, scopes.scopes[0]!)!;
  return {
    prepared,
    order: prepared.initial,
    hidden: hideReorderChip(prepared, prepared.initial, held)!,
  };
}

test('middle singleton vacancy is the genuine blank rather than the following populated row', () => {
  const { order, hidden } = operands('aaaaaaaaaa\nb\nc', 1);
  assert.deepEqual(valueReorderVacancy(order, hidden, 1), {
    kind: 'gap',
    gap: 0,
    blank: 0,
  });
  assert.deepEqual(valueReorderLayout(hidden).gaps[0]!.blankOffsets, [1]);
});

test('adjacent departure crosses an adjacent chip while preserving genuine blank destinations', () => {
  const { hidden } = operands('a\nb\nc', 1),
    origin = { kind: 'gap' as const, gap: 0, blank: 0 };
  assert.deepEqual(
    valueReorderAdjacentTarget(hidden, origin, { kind: 'row', row: 1, insertion: 0 }, 1),
    { kind: 'row', row: 1, insertion: 1 },
  );
  assert.deepEqual(
    valueReorderAdjacentTarget(hidden, origin, { kind: 'row', row: 0, insertion: 1 }, -1),
    { kind: 'row', row: 0, insertion: 0 },
  );
  assert.equal(
    valueReorderAdjacentTarget(hidden, { kind: 'row', row: 0, insertion: 1 }, origin, 1),
    origin,
  );
  assert.equal(
    valueReorderAdjacentTarget(hidden, origin, { kind: 'row', row: 8, insertion: 0 }, 1),
    null,
  );
});

for (const ending of ['\n', '\r', '\r\n'])
  test(`edge vacancies preserve separator UTF-16 topology, ending length=${ending.length}`, () => {
    const trailing = operands(`🦊,${ending}last`, 1),
      leading = operands(`first${ending}🦊`, 0);
    assert.deepEqual(valueReorderVacancy(trailing.order, trailing.hidden, 1), {
      kind: 'gap',
      gap: 0,
      blank: 1,
    });
    assert.deepEqual(valueReorderVacancy(leading.order, leading.hidden, 0), {
      kind: 'gap',
      gap: 0,
      blank: 0,
    });
  });

test('surviving chips resolve their current hidden row and insertion', () => {
  const { order, hidden } = operands('first\na, b, c\nlast', 2);
  assert.deepEqual(valueReorderVacancy(order, hidden, 2), {
    kind: 'row',
    row: 1,
    insertion: 1,
  });
});

test('empty regional vacancy preserves its partition identity', () => {
  const { order, hidden } = operands('a\n[SEP|Face]\nb', 0);
  assert.deepEqual(valueReorderVacancy(order, hidden, 0), {
    kind: 'partition',
    partition: 0,
  });
});

test('existing blank ordinals identify the held physical line', () => {
  const { order, hidden } = operands('a\n\nb\n\nc', 1);
  assert.deepEqual(valueReorderVacancy(order, hidden, 1), {
    kind: 'gap',
    gap: 0,
    blank: 1,
  });
});

test('foreign hidden order and unremoved or absent held chip cannot supply an origin', () => {
  const { order, hidden } = operands('a, b, c', 1);
  assert.equal(valueReorderVacancy(order, order, 1), null);
  assert.equal(valueReorderVacancy(order, { ...hidden, order: [2, 0] }, 1), null);
  assert.equal(valueReorderVacancy(order, hidden, 8), null);
});

for (const specimen of [
  { insertion: 0, direction: 1 as const, expected: 'a\n\nc, b, d, e' },
  { insertion: 1, direction: 1 as const, expected: 'a\n\nc, b, d, e' },
  { insertion: 2, direction: -1 as const, expected: 'a\n\nc, d, b, e' },
  { insertion: 3, direction: 1 as const, expected: 'a\n\nc, d, e, b' },
  { insertion: 3, direction: -1 as const, expected: 'a\n\nc, d, b, e' },
  { insertion: 0, direction: -1 as const, expected: 'a\n\nb, c, d, e' },
])
  test(`adjacent row boundary source contract insertion=${specimen.insertion}, direction=${specimen.direction}`, () => {
    const { prepared, hidden } = operands('a\nb\nc, d, e', 1),
      origin = { kind: 'gap' as const, gap: 0, blank: 0 },
      candidate = {
        kind: 'row' as const,
        row: 1,
        insertion: specimen.insertion,
      },
      target = valueReorderAdjacentTarget(hidden, origin, candidate, specimen.direction)!;
    assert.ok(target);
    const dropped = dropReorderChip(prepared, hidden, 1, target)!;
    assert.equal(serializeValueReorder(prepared, dropped)!.text, specimen.expected);
    assert.deepEqual(
      valueReorderAdjacentTarget(hidden, candidate, candidate, specimen.direction),
      candidate,
    );
  });

test('empty-partition departure crosses one near chip and retains its region header', () => {
  const { prepared, hidden } = operands('b\n[SEP|Face]\nc, d', 0),
    target = valueReorderAdjacentTarget(
      hidden,
      { kind: 'partition', partition: 0 },
      { kind: 'row', row: 0, insertion: 0 },
      1,
    )!;
  assert.ok(target);
  const dropped = dropReorderChip(prepared, hidden, 0, target)!;
  assert.equal(serializeValueReorder(prepared, dropped)!.text, '\n[SEP|Face]\nc, b, d');
});
