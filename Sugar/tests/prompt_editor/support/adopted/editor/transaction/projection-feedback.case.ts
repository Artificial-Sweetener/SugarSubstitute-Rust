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
/** Atomic feedback proof observes actual native decoration reuse and current-source retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type EditorState } from '@codemirror/state';
import { undo, undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import {
  wheelFeedback,
  showWheelFeedback,
} from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import {
  installWeightSession,
  retireWeightSession,
  weightSession,
} from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { decorateWeightFeedback } from '../../../../../../frontend/sugarprompt/editor/src/weight-feedback-decoration.ts';
import { setExactWeightDraft } from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import type { EditorView } from '@codemirror/view';
import { weightStateFor } from './weight-command-state.ts';

/** Observe immutable widgets through the actual native range owner, without manufacturing expected decorations. */
function widgets(state: EditorState): readonly LoraWidget[] {
  const result: LoraWidget[] = [];
  state.field(richProjection).all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof LoraWidget) result.push(value.spec.widget);
  });
  return result;
}

test('atomic readiness replaces only the targeted widget, reuses real output on equivalent publication and retires on source change', () => {
  let state = weightStateFor('<lora:real:1.20>, <lora:missing:0.50>');
  state = state.update({
    effects: StateEffect.appendConfig.of(wheelFeedback),
  }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  const original = widgets(state);
  const ready = {
    kind: 'lora',
    range: { from: 0, to: 16 },
    freshness: state.field(editorFreshness),
  } as const;
  state = state.update({ effects: showWheelFeedback.of(ready) }).state;
  assert.equal(widgets(state)[0]!.adjustment, 'wheel');
  assert.equal(widgets(state)[1], original[1]);
  const prepared = state.field(richProjection);
  state = state.update({ effects: showWheelFeedback.of(ready) }).state;
  assert.equal(state.field(richProjection), prepared);
  for (const changes of [
    { from: 0, insert: 'x' },
    { from: 11, to: 15, insert: '10.00' },
  ]) {
    const mapped = state.update({ changes }).state;
    assert.equal(mapped.field(wheelFeedback).target, null);
    assert.ok(widgets(mapped).every((widget) => widget.adjustment === 'none'));
  }
  state = state.update({ effects: showWheelFeedback.of(null) }).state;
  assert.equal(widgets(state)[0]!.adjustment, 'none');
  assert.equal(widgets(state)[1], original[1]);
  state = state.update({
    changes: { from: state.doc.length, insert: 'x' },
    effects: showWheelFeedback.of(ready),
  }).state;
  assert.equal(state.field(wheelFeedback).target, null);
  assert.ok(widgets(state).every((widget) => widget.adjustment === 'none'));
});

test('kind-qualified emphasis feedback accents only actual boundary widgets and retains nested LoRA output', () => {
  const source = '(outer (<lora:x:1.00>:1.05):1.20), {animals|2}';
  let state = weightStateFor(source);
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  const all = state.field(richProjection).all;
  const range = {
    from: source.indexOf('<lora:'),
    to: source.indexOf(':1.05)'),
  };
  const before: {
    from: number;
    to: number;
    widget: EmphasisWidget | LoraWidget;
  }[] = [];
  all.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.widget instanceof EmphasisWidget || value.spec.widget instanceof LoraWidget)
      before.push({ from, to, widget: value.spec.widget });
  });
  const freshness = state.field(editorFreshness);
  state = state.update({
    effects: showWheelFeedback.of({ kind: 'emphasis', range, freshness }),
  }).state;
  const ready = state.field(richProjection),
    after: typeof before = [];
  ready.all.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.widget instanceof EmphasisWidget || value.spec.widget instanceof LoraWidget)
      after.push({ from, to, widget: value.spec.widget });
  });
  assert.equal(range.from, 8);
  assert.equal(range.to, 21);
  assert.equal(after.length, 5);
  for (const row of after) {
    const original = before.find((value) => value.from === row.from && value.to === row.to)!;
    if (row.widget instanceof EmphasisWidget && (row.to === 8 || row.from === 21)) {
      assert.equal(row.widget.feedback, 1);
      assert.equal(row.widget.token, original.widget.token);
      assert.notEqual(row.widget, original.widget);
    } else assert.equal(row.widget, original.widget);
  }
  assert.equal(widgets(state)[0]!.adjustment, 'none');
  assert.equal(
    decorateWeightFeedback(ready.all, { kind: 'emphasis', range }, true, false),
    ready.all,
  );
  state = state.update({
    effects: installWeightSession.of({
      kind: 'emphasis',
      range,
      owner: 'pointer',
      freshness,
      boundary: 'end',
      neutral: false,
    }),
  }).state;
  const adjusting = state.field(richProjection).all;
  assert.equal(state.field(weightSession)!.decorations.size, 0);
  const current: EmphasisWidget[] = [];
  adjusting.between(range.from, range.to, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget && value.spec.widget.feedback)
      current.push(value.spec.widget);
  });
  assert.deepEqual(
    current.map((widget) => widget.feedback),
    [3, 3],
  );
  const key = Symbol();
  state = state.update({
    effects: setExactWeightDraft.of({
      kind: 'emphasis',
      range,
      freshness,
      key,
      phase: 'editing',
    }),
  }).state;
  const inputs: EmphasisWidget[] = [];
  state.field(richProjection).decorations.between(range.to, range.to, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget && value.spec.widget.part === 'suffix')
      inputs.push(value.spec.widget);
  });
  assert.equal(inputs.length, 1);
  assert.equal(inputs[0]!.draftKey, key);
  assert.equal(inputs[0]!.feedback, 3);
  state = state.update({ effects: showWheelFeedback.of(null) }).state;
  const updatedInputs: EmphasisWidget[] = [];
  state.field(richProjection).decorations.between(range.to, range.to, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget && value.spec.widget.part === 'suffix')
      updatedInputs.push(value.spec.widget);
  });
  assert.equal(updatedInputs[0]!.draftKey, key);
  assert.equal(updatedInputs[0]!.feedback, 2);
  const writes: [string, boolean][] = [],
    children = Object.freeze([{}, {}]);
  const dom = Object.freeze({
    children,
    classList: {
      toggle: (name: string, force: boolean): boolean => {
        writes.push([name, force]);
        return force;
      },
    },
  });
  assert.equal(
    updatedInputs[0]!.updateDOM(dom as unknown as HTMLElement, {} as EditorView, inputs[0]!),
    true,
  );
  assert.equal(dom.children, children);
  assert.deepEqual(writes, [
    ['sp-emphasis-accent', true],
    ['sp-wheel-ready', false],
    ['sp-adjustment-content', true],
  ]);
  state = state.update({
    effects: [showWheelFeedback.of(null), retireWeightSession.of(null)],
  }).state;
  assert.equal(state.field(richProjection).accented.length, 0);
  assert.equal(widgets(state)[0]!.adjustment, 'none');
  const retired: EmphasisWidget[] = [];
  state.field(richProjection).all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget) retired.push(value.spec.widget);
  });
  assert.ok(retired.every((widget) => widget.feedback === 0));
});

test('pointer adjustment and wheel readiness have independent source-backed targets and prepared feedback', () => {
  let state = weightStateFor('<lora:real:1.20>, <lora:missing:0.50>');
  state = state.update({
    effects: StateEffect.appendConfig.of(wheelFeedback),
  }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  const freshness = state.field(editorFreshness);
  state = state.update({
    effects: [
      showWheelFeedback.of({
        kind: 'lora',
        range: { from: 0, to: 16 },
        freshness,
      }),
      installWeightSession.of({
        owner: 'pointer',
        kind: 'lora',
        range: { from: 18, to: 37 },
        freshness,
        boundary: 'end',
        neutral: false,
      }),
    ],
  }).state;
  assert.deepEqual(
    widgets(state).map((widget) => widget.adjustment),
    ['wheel', 'pointer'],
  );
  assert.deepEqual(state.field(richProjection).accented, [
    { kind: 'lora', range: { from: 18, to: 37 } },
    { kind: 'lora', range: { from: 0, to: 16 } },
  ]);
});

/** Actual neutral session output can contain distinct prefix/suffix point widgets at the same logical position. */
test('feedback deduplicates actual decoration identity while preserving coincident neutral roles', () => {
  let neutral = weightStateFor('');
  neutral = neutral.update({
    effects: installWeightSession.of({
      kind: 'emphasis',
      range: { from: 0, to: 0 },
      owner: 'pointer',
      freshness: neutral.field(editorFreshness),
      boundary: 'end',
      neutral: true,
    }),
  }).state;
  const session = neutral.field(weightSession)!;
  assert.deepEqual(session.range, { from: 0, to: 0 });
  assert.equal(session.neutral, true);
  const original = session.decorations;
  /** Observe the actual public RangeSet order and widget roles, retaining their distinct output identities. */
  const roles = (decorations: typeof original) => {
    const values: { from: number; to: number; widget: EmphasisWidget }[] = [];
    for (const cursor = decorations.iter(); cursor.value; cursor.next()) {
      const widget = cursor.value.spec.widget;
      assert.ok(widget instanceof EmphasisWidget);
      values.push({ from: cursor.from, to: cursor.to, widget });
    }
    return values;
  };
  const before = roles(original);
  assert.deepEqual(
    before.map(({ from, to, widget }) => [from, to, widget.part, widget.feedback]),
    [
      [0, 0, 'prefix', 2],
      [0, 0, 'suffix', 2],
    ],
  );
  const combined = decorateWeightFeedback(original, session, true, true);
  const ready = roles(combined);
  assert.equal(combined.size, 2);
  assert.deepEqual(
    ready.map(({ from, to, widget }) => [from, to, widget.part, widget.feedback]),
    [
      [0, 0, 'prefix', 3],
      [0, 0, 'suffix', 3],
    ],
  );
  for (const [index, role] of ready.entries()) {
    assert.equal(role.widget.token, before[index]!.widget.token);
    assert.notEqual(role.widget, before[index]!.widget);
  }
  assert.equal(decorateWeightFeedback(combined, session, true, true), combined);
  const retired = decorateWeightFeedback(combined, session, false, false);
  assert.equal(retired.size, 2);
  assert.deepEqual(
    roles(retired).map(({ from, to, widget }) => [from, to, widget.part, widget.feedback]),
    [
      [0, 0, 'prefix', 0],
      [0, 0, 'suffix', 0],
    ],
  );
  assert.equal(decorateWeightFeedback(retired, session, false, false), retired);
  neutral = neutral.update({ effects: retireWeightSession.of(null) }).state;
  assert.equal(neutral.field(weightSession), null);
  assert.equal(neutral.doc.length, 0);

  let atomic = weightStateFor('<lora:real:1.20>');
  atomic = atomic.update({
    effects: installProjection.of(prepareProjection(atomic, prepareNativeStructure(atomic))),
  }).state;
  const prepared = atomic.field(richProjection).all;
  const originalLora = widgets(atomic)[0]!;
  const once = decorateWeightFeedback(
    prepared,
    { kind: 'lora', range: { from: 0, to: 16 } },
    true,
    false,
  );
  const observed: LoraWidget[] = [];
  once.between(0, 16, (_from, _to, value) => {
    if (value.spec.widget instanceof LoraWidget) observed.push(value.spec.widget);
  });
  assert.equal(once.size, prepared.size);
  assert.equal(observed.length, 1);
  assert.equal(observed[0]!.adjustment, 'wheel');
  assert.equal(observed[0]!.token, originalLora.token);
  assert.equal(observed[0]!.resource, originalLora.resource);
  assert.notEqual(observed[0], originalLora);
  assert.equal(
    decorateWeightFeedback(once, { kind: 'lora', range: { from: 0, to: 16 } }, true, false),
    once,
  );
});

/** Source changes retire both mapped boundary accents before any new semantic/projection publication. */
test('emphasis interior endpoint edits retire actual mapped feedback immediately', () => {
  const cases = [
    {
      changes: { from: 1, insert: 'x' },
      source: '(xcat:1.05)',
      bounds: [
        [0, 1],
        [5, 11],
      ],
    },
    {
      changes: { from: 4, insert: 'x' },
      source: '(catx:1.05)',
      bounds: [
        [0, 1],
        [5, 11],
      ],
    },
    {
      changes: { from: 1, to: 4, insert: '' },
      source: '(:1.05)',
      bounds: [
        [0, 1],
        [1, 7],
      ],
    },
  ] as const;
  for (const specimen of cases) {
    let state = weightStateFor('(cat:1.05)');
    state = state.update({
      effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
    }).state;
    const freshness = state.field(editorFreshness);
    state = state.update({
      effects: [
        showWheelFeedback.of({
          kind: 'emphasis',
          range: { from: 1, to: 4 },
          freshness,
        }),
        installWeightSession.of({
          kind: 'emphasis',
          range: { from: 1, to: 4 },
          owner: 'pointer',
          freshness,
          boundary: 'end',
          neutral: false,
        }),
      ],
    }).state;
    /** The actual public ordered output supplies nonvacuous prefix/suffix witnesses. */
    const roles = (current: EditorState) => {
      const output: { from: number; to: number; widget: EmphasisWidget }[] = [];
      for (const cursor = current.field(richProjection).all.iter(); cursor.value; cursor.next()) {
        const widget = cursor.value.spec.widget;
        if (widget instanceof EmphasisWidget)
          output.push({ from: cursor.from, to: cursor.to, widget });
      }
      return output;
    };
    const before = roles(state);
    assert.deepEqual(
      before.map(({ from, to, widget }) => [from, to, widget.part, widget.feedback]),
      [
        [0, 1, 'prefix', 3],
        [4, 10, 'suffix', 3],
      ],
    );
    assert.equal(readExactSource(state), '(cat:1.05)');
    assert.equal(undoDepth(state), 0);
    const selection = state.selection;
    const transaction = state.update({ changes: specimen.changes });
    state = transaction.state;
    assert.equal(state.field(weightSession), null);
    assert.equal(state.field(wheelFeedback).target, null);
    assert.equal(state.field(richProjection).sourceIdentity, null);
    assert.deepEqual(state.field(richProjection).accented, []);
    const after = roles(state);
    assert.deepEqual(
      after.map(({ from, to }) => [from, to]),
      specimen.bounds,
    );
    assert.deepEqual(
      after.map(({ widget }) => [widget.part, widget.feedback]),
      [
        ['prefix', 0],
        ['suffix', 0],
      ],
    );
    for (const [index, role] of after.entries())
      assert.equal(role.widget.token, before[index]!.widget.token);
    assert.equal(readExactSource(state), specimen.source);
    assert.equal(state.selection.eq(selection.map(transaction.changes)), true);
    assert.equal(undoDepth(state), 1);
    assert.equal(redoDepth(state), 0);
    assert.equal(
      undo({
        state,
        dispatch: (transaction) => {
          state = transaction.state;
        },
      }),
      true,
    );
    assert.equal(readExactSource(state), '(cat:1.05)');
    assert.equal(undoDepth(state), 0);
    assert.equal(redoDepth(state), 1);
  }
});
