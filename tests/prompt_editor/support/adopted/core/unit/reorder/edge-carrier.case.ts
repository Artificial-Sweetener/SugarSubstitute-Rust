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
/** Canonical carrier controls prove source-owned punctuation destinations and exact lift/restore topology. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import {
  prepareValueReorder,
  hideReorderChip,
  dropReorderChip,
  valueReorderLayout,
  valueReorderVacancy,
  serializeValueReorder,
} from '../../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Actual public Core preparation supplies canonical source and immutable carrier provenance. */
function prepare(source: string) {
  const scopes = prepareDocumentPromptScopes(prepareDocumentValues(source, 'prompt'), {
    style: 'curly',
  });
  return prepareValueReorder(scopes, scopes.scopes[0]!)!;
}

for (const [ordinal, specimen] of [
  { source: 'a\nb,', blank: 1 },
  { source: 'a\nb,\n', blank: 1 },
  { source: 'a\nb,  ', blank: 1 },
  { source: 'a\nb,\n\n', blank: 1 },
  { source: 'a\n\nb,', blank: 2 },
  { source: 'a\nb', blank: 1 },
  { source: '🦊\r\nb,\r\n', blank: 1 },
  { source: 'a\rb,', blank: 1 },
].entries())
  test(`authored edge punctuation retains the exact hidden vacancy and restoration, specimen=${ordinal}`, () => {
    const prepared = prepare(specimen.source),
      order = prepared.initial,
      held = prepared.chips.length - 1,
      hidden = hideReorderChip(prepared, order, held)!;
    assert.ok(hidden);
    const vacancy = valueReorderVacancy(order, hidden, held);
    assert.deepEqual(vacancy, { kind: 'gap', gap: 0, blank: specimen.blank });
    const gap = valueReorderLayout(hidden).gaps[0]!;
    assert.equal(gap.carrier, 'suffix');
    assert.equal(gap.text, hidden.suffix.text);
    assert.equal(
      gap.blankOffsets[specimen.blank],
      hidden.suffix.text.length - order.suffix.text.length,
    );
    const restored = dropReorderChip(prepared, hidden, held, vacancy!)!;
    assert.equal(serializeValueReorder(prepared, restored)!.text, specimen.source);
    assert.equal(restored.suffix.text, order.suffix.text);
    assert.equal(Object.isFrozen(hidden.suffix.pieces), true);
    assert.equal(hidden.suffix.scope, prepared.scope);
    assert.equal(serializeValueReorder(prepared, order)!.text, specimen.source);
  });

test('comma-bearing edge destinations preserve existing blank ordinals and repeat without syntax accumulation', () => {
  const source = 'a\n\nb,  \n\n',
    prepared = prepare(source),
    held = 1;
  let order = prepared.initial;
  for (let round = 0; round < 8; round++) {
    const hidden = hideReorderChip(prepared, order, held)!;
    assert.deepEqual(valueReorderLayout(hidden).gaps[0]!.blankOffsets, [0, 1, 2, 6, 7]);
    const vacancy = valueReorderVacancy(order, hidden, held)!;
    assert.deepEqual(vacancy, { kind: 'gap', gap: 0, blank: 2 });
    order = dropReorderChip(prepared, hidden, held, vacancy)!;
    assert.equal(serializeValueReorder(prepared, order)!.text, source);
  }
});

test('structural regional suffix remains owned by partition topology rather than a comma-only edge gap', () => {
  const prepared = prepare('a\n[SEP|Face]\nb,\n[SEP|Tail]\n'),
    hidden = hideReorderChip(prepared, prepared.initial, 1)!;
  assert.equal(
    valueReorderLayout(hidden).gaps.some((gap) => gap.carrier === 'suffix'),
    false,
  );
  assert.deepEqual(valueReorderVacancy(prepared.initial, hidden, 1), {
    kind: 'partition',
    partition: 1,
  });
});

test('comma before a newline retains its established valid vacancy companion', () => {
  const prepared = prepare('a,\nb'),
    hidden = hideReorderChip(prepared, prepared.initial, 1)!;
  assert.deepEqual(valueReorderVacancy(prepared.initial, hidden, 1), {
    kind: 'gap',
    gap: 0,
    blank: 1,
  });
});
