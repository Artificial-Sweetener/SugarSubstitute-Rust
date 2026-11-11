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
/** Independent canonical plan provenance cannot adopt reconstructed captures, targets or receipts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  prepareNativeReorder,
  prepareReorderAuthorship,
  prepareReorderCommit,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import {
  prepareNativeReorderPlan,
  prepareReorderProvenance,
  reorderProvenanceSource,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-provenance.ts';
import {
  authorship,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparedAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { prepareInspectionStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { captureReorder, reorderState } from './reorder-fixture.ts';

test('foreign capture facade and assembled canonical plan never grant authored provenance', () => {
  const { state } = reorderState('a, b'),
    capture = captureReorder(state);
  assert.equal(prepareReorderAuthorship(state, { ...capture }, capture.prepared.initial), null);
  assert.equal(prepareNativeReorderPlan(state, capture.context, { ...capture.target }), null);
  assert.equal(
    prepareReorderProvenance(state, { ...capture.plan }, capture.prepared.initial),
    null,
  );
  assert.equal(prepareReorderCommit(state, capture, capture.prepared.initial).status, 'unchanged');
});
test('actual canonical target source selection permission and authored state remain prerequisites', () => {
  const fixture = reorderState('a, b'),
    { state } = fixture,
    capture = captureReorder(state);
  for (const changed of [
    state.update({ changes: { from: 0, insert: 'x' } }).state,
    state.update({ selection: { anchor: 1 } }).state,
    state.update({
      effects: installAuthorship.of(preparedAuthorship([], [{ from: 0, to: 1 }])),
    }).state,
    state.update({
      effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
    }).state,
  ]) {
    assert.equal(prepareReorderProvenance(changed, capture.plan, capture.prepared.initial), null);
    if (changed.field(authorship) === state.field(authorship))
      assert.equal(prepareNativeReorderPlan(changed, capture.context, capture.target), null);
  }
  assert.equal(
    prepareReorderAuthorship(state, capture, capture.prepared.initial)?.output.text,
    'a, b',
  );
});
test('metadata-only preview state can prepare genuine provenance but cannot borrow another immutable-state receipt', () => {
  const { state } = reorderState('a, b'),
    capture = captureReorder(state);
  const later = state.update({}).state;
  assert.notEqual(later, state);
  const prepared = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
  assert.equal(
    reorderProvenanceSource(later, capture.target, prepared.provenance, {
      range: { from: 0, to: 4 },
      text: 'a, b',
    }),
    null,
  );
  assert.equal(
    reorderProvenanceSource(state, capture.target, prepared.provenance, {
      range: { from: 0, to: 4 },
      text: 'a, b',
    }),
    null,
  );
  const fresh = prepareReorderAuthorship(later, capture, capture.prepared.initial)!;
  assert.equal(
    reorderProvenanceSource(later, capture.target, fresh.provenance, {
      range: { from: 0, to: 4 },
      text: 'a, b',
    })?.output,
    fresh.output,
  );
});
test('receipt identity target complete replacement and actual output are all exact', () => {
  for (const variant of ['foreign', 'target', 'range', 'output'] as const) {
    const { state } = reorderState('a, b'),
      capture = captureReorder(state);
    const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    const prepared = prepareReorderAuthorship(state, capture, order)!;
    const result = reorderProvenanceSource(
      state,
      variant === 'target' ? { ...capture.target } : capture.target,
      variant === 'foreign' ? { ...prepared.provenance } : prepared.provenance,
      {
        range: { from: 0, to: variant === 'range' ? 3 : 4 },
        text: prepared.output.text + (variant === 'output' ? 'x' : ''),
      },
    );
    assert.equal(result, null);
    assert.equal(prepareReorderCommit(state, capture, order).status, 'ready');
  }
});

test('another genuinely prepared inspection profile cannot be adopted as a native gesture capture', () => {
  const { state } = reorderState('a, b');
  const inspected = prepareInspectionStructure(state, {
    style: 'double-underscore',
  });
  assert.equal(
    prepareNativeReorder(
      state,
      {
        values: inspected.nativeValues,
        semantics: inspected.documentSemantics,
        enabled: true,
        composing: false,
      },
      { from: 0, to: 0 },
    ),
    null,
  );
  const capture = captureReorder(state);
  assert.equal(prepareReorderCommit(state, capture, capture.prepared.initial).status, 'unchanged');
});
test('observed authored payload mutation permanently retires only its original receipt', () => {
  const fixture = reorderState('a, b'),
    authored = { ...preparedAuthorship([], []) };
  const state = fixture.state.update({
    effects: installAuthorship.of(authored),
  }).state;
  const capture = captureReorder(state),
    prepared = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
  const original = authored.literals;
  authored.literals = preparedAuthorship([], [{ from: 0, to: 1 }]).literals;
  const replacement = { range: { from: 0, to: 4 }, text: 'a, b' };
  assert.equal(
    reorderProvenanceSource(state, capture.target, prepared.provenance, replacement),
    null,
  );
  authored.literals = original;
  assert.equal(
    reorderProvenanceSource(state, capture.target, prepared.provenance, replacement),
    null,
  );
  const fresh = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
  assert.equal(
    reorderProvenanceSource(state, capture.target, fresh.provenance, replacement)?.output,
    fresh.output,
  );
});

test('exposed receipt output cannot authorize foreign text through genuine issuance', () => {
  const { state } = reorderState('a, b'),
    capture = captureReorder(state);
  const prepared = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
  const source = reorderProvenanceSource(state, capture.target, prepared.provenance, {
    range: { from: 0, to: 4 },
    text: prepared.output.text,
  })!;
  assert.equal(Object.isFrozen(source), true);
  assert.equal(
    Reflect.set(source, 'output', Object.freeze({ ...prepared.output, text: 'x, y' })),
    false,
  );
  assert.equal(source.output, prepared.output);
  const refused = prepareDocumentValueEdit(state, {
    context: capture.context,
    target: capture.target,
    range: { from: 0, to: 4 },
    text: 'x, y',
    selection: {
      ranges: [{ kind: 'decoded', anchor: 0, head: 0 }],
      mainIndex: 0,
    },
    publication: { purpose: 'reorder', reorder: prepared.provenance },
  });
  assert.equal(refused.status, 'rejected');
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  })!;
  const following = prepareReorderCommit(state, capture, order);
  assert.equal(following.status, 'ready');
  assert.equal(currentDocumentValueEdit(state, capture.context, following.edit), null);
  assert.equal(readExactSource(following.edit.transactions[0]!.state), 'b, a');
});

test('exposed receipt state cannot adopt a later genuine metadata-only state', () => {
  const { state } = reorderState('a, b'),
    capture = captureReorder(state);
  const prepared = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
  const replacement = { range: { from: 0, to: 4 }, text: prepared.output.text };
  const source = reorderProvenanceSource(state, capture.target, prepared.provenance, replacement)!;
  const later = state.update({}).state;
  assert.equal(Reflect.set(source, 'state', later), false);
  const refused = prepareDocumentValueEdit(later, {
    context: capture.context,
    target: capture.target,
    range: { from: 0, to: 4 },
    text: prepared.output.text,
    selection: {
      ranges: [{ kind: 'decoded', anchor: 0, head: 0 }],
      mainIndex: 0,
    },
    publication: { purpose: 'reorder', reorder: prepared.provenance },
  });
  assert.equal(refused.status, 'rejected');
  assert.equal(
    reorderProvenanceSource(later, capture.target, prepared.provenance, replacement),
    null,
  );
  const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 1,
  })!;
  const following = prepareReorderCommit(later, capture, order);
  assert.equal(following.status, 'ready');
  assert.equal(currentDocumentValueEdit(later, capture.context, following.edit), null);
  assert.equal(readExactSource(following.edit.transactions[0]!.state), 'b, a');
});

test('exposed receipt target authored index and snapshot descriptors retain original identities', () => {
  for (const field of ['target', 'authored', 'prepared', 'generated', 'literals'] as const) {
    const { state } = reorderState('a, b'),
      capture = captureReorder(state);
    const prepared = prepareReorderAuthorship(state, capture, capture.prepared.initial)!;
    const replacement = {
      range: { from: 0, to: 4 },
      text: prepared.output.text,
    };
    const source = reorderProvenanceSource(
      state,
      capture.target,
      prepared.provenance,
      replacement,
    )!;
    const original = source[field];
    const foreign =
      field === 'target'
        ? { ...capture.target }
        : field === 'authored'
          ? preparedAuthorship([], [{ from: 0, to: 1 }])
          : field === 'prepared'
            ? { ...capture.prepared }
            : [];
    assert.equal(Reflect.set(source, field, foreign), false);
    assert.equal(Reflect.defineProperty(source, field, { value: foreign }), false);
    assert.equal(source[field], original);
    assert.equal(
      reorderProvenanceSource(state, capture.target, prepared.provenance, replacement),
      source,
    );
    const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!;
    const following = prepareReorderCommit(state, capture, order);
    assert.equal(following.status, 'ready');
    assert.equal(currentDocumentValueEdit(state, capture.context, following.edit), null);
    assert.equal(readExactSource(following.edit.transactions[0]!.state), 'b, a');
  }
});
