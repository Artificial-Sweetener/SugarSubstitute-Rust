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
/** Accepted canonical mutations supply current feedback spelling; same-kind peers never authorize a fallback. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { weightPreviewText } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview-value.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth } from '@codemirror/commands';

for (const specimen of [
  {
    source: '(cat:1.05), (dog:2.00)',
    range: { from: 13, to: 16 },
    kind: 'emphasis' as const,
    value: '2.05',
  },
  {
    source: '<lora:model:1.05:0.3>',
    range: { from: 0, to: 21 },
    kind: 'lora' as const,
    value: '1.10',
  },
  {
    source: '{forest|9}',
    range: { from: 0, to: 10 },
    kind: 'wildcard' as const,
    value: '10',
  },
  {
    source: '🦊,\r\n(cat:1.05)',
    range: { from: 5, to: 8 },
    kind: 'emphasis' as const,
    value: '1.10',
  },
])
  test(`accepted ${specimen.kind} feedback uses exact prepared target and native source mapping`, () => {
    const initial = weightStateFor(specimen.source),
      transactions = prepareWeightChange(
        initial,
        { kind: 'step', direction: 'increase' },
        {
          owner: 'pointer',
          freshness: initial.field(editorFreshness),
          kind: specimen.kind,
          range: specimen.range,
        },
      );
    assert.equal(transactions.length, 2);
    const state = transactions.at(-1)!.state,
      session = state.field(weightSession)!;
    assert.ok(session);
    const source = readExactSource(state),
      selection = state.selection,
      history = undoDepth(state),
      prepared = state.field(semanticStructures).native;
    assert.equal(weightPreviewText(state, session), specimen.value);
    assert.equal(weightPreviewText(state, { ...session }), null);
    const changed = state.update({ changes: { from: 0, insert: 'z' } }).state;
    assert.equal(weightPreviewText(changed, session), null);
    assert.equal(readExactSource(state), source);
    assert.equal(state.selection, selection);
    assert.equal(undoDepth(state), history);
    assert.equal(state.field(semanticStructures).native, prepared);
  });

test('neutral unwrap exposes its accepted 1.00 value while retaining the unwrapped source', () => {
  const initial = weightStateFor('(cat:1.05)'),
    state = prepareWeightChange(
      initial,
      { kind: 'step', direction: 'decrease' },
      {
        owner: 'pointer',
        freshness: initial.field(editorFreshness),
        kind: 'emphasis',
        range: { from: 1, to: 4 },
      },
    ).at(-1)!.state,
    session = state.field(weightSession)!;
  assert.ok(session.neutral);
  assert.equal(weightPreviewText(state, session), '1.00');
  assert.equal(readExactSource(state), 'cat');
});
