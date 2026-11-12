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
import {
  semanticStructures,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
/** Native LoRA command contracts independently establish target continuation, caret mapping and exact history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redo, undo, undoDepth } from '@codemirror/commands';
import { EditorSelection, EditorState, StateEffect, Transaction } from '@codemirror/state';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { weightStateFor } from './weight-command-state.ts';

test('LoRA pointer steps retain an outer target across numeric growth and map reverse outside selection through native history', () => {
  const source = '🙂\r\n<lora:real:9.95:0.37:lbw=IN02>, tail';
  const logical = source.replace('\r\n', '\n');
  const from = logical.indexOf('<'),
    to = logical.indexOf('>') + 1;
  const initial = weightStateFor(source, logical.length, to + 2);
  const request = {
    owner: 'pointer',
    kind: 'lora',
    range: { from, to },
    freshness: initial.field(editorFreshness),
  } as const;
  let state = prepareWeightChange(initial, { kind: 'step', direction: 'increase' }, request).at(
    -1,
  )!.state;
  assert.equal(readExactSource(state), source.replace('9.95', '10.00'));
  assert.deepEqual(
    [state.selection.main.anchor, state.selection.main.head],
    [logical.length + 1, to + 3],
  );
  assert.deepEqual(state.field(weightSession)!.range, { from, to: to + 1 });
  assert.equal(state.field(weightSession)!.kind, 'lora');
  assert.equal(state.field(weightSession)!.neutral, false);
  assert.equal(undoDepth(state), 1);
  const continued = prepareWeightChange(
    state,
    { kind: 'step', direction: 'increase' },
    {
      ...request,
      freshness: state.field(editorFreshness),
      range: state.field(weightSession)!.range,
    },
  ).at(-1)!.state;
  assert.equal(readExactSource(continued), source.replace('9.95', '10.05'));
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), source);
  assert.ok(state.selection.eq(initial.selection));
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), source.replace('9.95', '10.00'));
  assert.deepEqual(
    [state.selection.main.anchor, state.selection.main.head],
    [logical.length + 1, to + 3],
  );
});

test('LoRA exact entry finishes after the complete token, retains secondary/block fields and adds no history for unchanged source', () => {
  const source = '🙂\r\n(<lora:real:1.20:0.37:lbw=IN02>:1.10), tail';
  const logical = source.replace('\r\n', '\n');
  const range = { from: logical.indexOf('<'), to: logical.indexOf('>') + 1 };
  const initial = weightStateFor(source, logical.length);
  const request = {
    owner: 'exact',
    kind: 'lora',
    range,
    freshness: initial.field(editorFreshness),
    neutral: false,
    completion: 'keyboard',
  } as const;
  const same = prepareWeightChange(initial, { kind: 'set', weight: '1.20' }, request).at(-1)!.state;
  assert.equal(readExactSource(same), source);
  assert.equal(same.selection.main.head, range.to);
  assert.equal(undoDepth(same), 0);
  const changed = prepareWeightChange(initial, { kind: 'set', weight: '-.5' }, request).at(
    -1,
  )!.state;
  assert.equal(readExactSource(changed), source.replace('real:1.20', 'real:-0.50'));
  assert.equal(changed.selection.main.head, range.to + 1);
  assert.equal(undoDepth(changed), 1);
  assert.deepEqual(prepareWeightChange(changed, { kind: 'set', weight: '2.00' }, request), []);
});

test('LoRA targets reject mismatched kind/ranges and read-only commands while crossing one retains token syntax', () => {
  const initial = weightStateFor('<lora:real:1.05>, tail', 21);
  const request = {
    owner: 'wheel',
    token: Symbol(),
    kind: 'lora',
    range: { from: 0, to: 16 },
    freshness: initial.field(editorFreshness),
  } as const;
  const neutral = prepareWeightChange(initial, { kind: 'step', direction: 'decrease' }, request).at(
    -1,
  )!.state;
  assert.equal(readExactSource(neutral), '<lora:real:1.00>, tail');
  assert.equal(neutral.field(weightSession)!.neutral, false);
  assert.deepEqual(
    prepareWeightChange(
      initial,
      { kind: 'step', direction: 'increase' },
      { ...request, range: { from: 1, to: 15 } },
    ),
    [],
  );
  assert.deepEqual(
    prepareWeightChange(
      initial,
      { kind: 'step', direction: 'increase' },
      { ...request, kind: 'emphasis' },
    ),
    [],
  );
  const blocked = initial.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  assert.deepEqual(
    prepareWeightChange(
      blocked,
      { kind: 'step', direction: 'increase' },
      { ...request, freshness: blocked.field(editorFreshness) },
    ),
    [],
  );
});

test('CSV LoRA pointer continuation uses genuine storage edits and native outside-value directional ChangeDesc/history', () => {
  const source = 'H,Z\r\n"<lora:real:9.95:0.37>, a""b,\r\n🙂","neighbor (soft:1.20)"\rlast,end\n';
  let state = weightStateFor(source, 0, 0, 'wildcard-csv');
  const prepared = state.field(semanticStructures).native!,
    entry = prepared.nativeValues.entries[2]!,
    neighbor = prepared.nativeValues.entries[3]!,
    span = entry.scope!.document.loraSpans[0]!,
    map = entry.semanticMapping!,
    outside = neighbor.semanticMapping!;
  state = state.update({
    selection: EditorSelection.create(
      [
        EditorSelection.range(outside.logicalRange.to, outside.logicalRange.from + 2),
        EditorSelection.cursor(state.doc.length),
      ],
      0,
    ),
  }).state;
  const originalSelection = state.selection;
  const request = {
    owner: 'pointer',
    kind: 'lora',
    freshness: state.field(editorFreshness),
    range: {
      from: map.decodedToLogical[span.outerRange.from]!,
      to: map.decodedToLogical[span.outerRange.to]!,
    },
  } as const;
  const transactions = prepareWeightChange(state, { kind: 'step', direction: 'increase' }, request);
  assert.equal(transactions.length, 2);
  assert.equal(transactions[0]!.annotation(Transaction.userEvent), 'input.weight');
  state = transactions.at(-1)!.state;
  assert.equal(readExactSource(state) === source.replace('9.95', '10.00'), true);
  assert.deepEqual(
    state.selection.ranges.map((range) => [range.anchor, range.head]),
    originalSelection.ranges.map((range) => [range.anchor + 1, range.head + 1]),
  );
  assert.equal(state.selection.mainIndex, originalSelection.mainIndex);
  assert.equal(state.field(weightSession)!.kind, 'lora');
  assert.equal(state.field(weightSession)!.range.to, request.range.to + 1);
  assert.equal(prepareNativeStructure(state), state.field(semanticStructures).native);
  assert.equal(undoDepth(state), 1);
  const selection = state.selection;
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state) === source, true);
  assert.equal(state.selection.eq(originalSelection), true);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(state.selection.eq(selection), true);
  state = transactions.at(-1)!.state;
  const continued = prepareWeightChange(
    state,
    { kind: 'step', direction: 'increase' },
    {
      ...request,
      freshness: state.field(editorFreshness),
      range: state.field(weightSession)!.range,
    },
  ).at(-1)!.state;
  assert.equal(readExactSource(continued) === source.replace('9.95', '10.05'), true);
  assert.equal(undoDepth(continued), 2);
});
