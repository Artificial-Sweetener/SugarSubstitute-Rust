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
/** Literal proposed heading cases prove Core classification and actual prepared-output reuse without claiming native geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  moveReorderChip,
  type ReorderState,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { prepareReorderScenePaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-scene-paint.ts';
import { prepareReorderWidgetPlan } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';
import { prepareSceneBands } from '../../../../../../frontend/sugarprompt/editor/src/scene-line-bands.ts';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { Decoration } from '@codemirror/view';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth } from '@codemirror/commands';
import { EditorSelection } from '@codemirror/state';
import { projectionState } from './projection-state.ts';
import { captureReorder, reorderState } from './reorder-fixture.ts';

/** Genuine native preparation supplies all source/profile/currentness identities; the explicit exact-source UTF-16 hit targets visible fixture content and its genuine native selection remains authoritative. */
function specimen(source: string, position = 0) {
  const initial = initializeNativeStructure(
    projectionState(source).update({
      selection: EditorSelection.cursor(position),
    }).state,
  );
  const state = initial.update({
    effects: installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
  }).state;
  const capture = captureReorder(state, position);
  return { state, capture };
}
/** Use the actual issued output/preparation path; source and history remain owned by the original state. */
function preview(f: ReturnType<typeof specimen>, order: ReorderState, chip: number) {
  const result = prepareReorderPreview(
    { capture: f.capture, order, token: {}, activeChip: chip },
    'complete',
  );
  assert.ok(result);
  return result;
}
/** Observe only closed counts and positions from the real compiled decorations. */
function roles(value: ReturnType<typeof prepareReorderScenePaint>, length: number) {
  let titles = 0,
    markers = 0,
    emphasis = 0;
  value.between(0, length, (_from, _to, decoration) => {
    if (decoration.spec.class === 'sp-scene-title') titles++;
    if (decoration.spec.scene === true && decoration.spec.widget) markers++;
    if (decoration.spec.widget instanceof EmphasisWidget) emphasis++;
  });
  return { titles, markers, emphasis };
}
test('unchanged canonical and repeated actual order borrow the exact prepared SceneDocument and plan', () => {
  const f = specimen('shared\n**One\nbody\n**Two\nlast'),
    first = preview(f, f.capture.prepared.initial, 0),
    second = preview(f, f.capture.prepared.initial, 0);
  assert.equal(first.scenes, f.capture.prepared.scope.sceneContexts!.document);
  assert.equal(second.scenes, first.scenes);
  assert.equal(prepareReorderScenePaint(first), prepareReorderScenePaint(first));
  assert.equal(prepareReorderWidgetPlan(first), prepareReorderWidgetPlan(first));
  assert.equal(readExactSource(f.state), 'shared\n**One\nbody\n**Two\nlast');
  assert.equal(undoDepth(f.state), 0);
});
/** Real mapped decoration identity proves avoided full-range rebuilding for a changed ordinary prompt, while literal roles and exact source/history remain independently expected. */
test('changed orders without headings reuse the actual ordinary paint output', (context) => {
  const source = 'first, (second:2)',
    f = specimen(source),
    chip = f.capture.active,
    order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
  assert.ok(order);
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, '(second:2), first');
  assert.equal(f.capture.prepared.scope.sceneContexts!.document.scenes.length, 0);
  assert.equal(actual.scenes!.scenes.length, 0);
  // TestContext owns restoration; the spy delegates to the actual CodeMirror factory and retains its real output.
  const sets = context.mock.method(Decoration, 'set'),
    paint = prepareReorderScenePaint(actual);
  assert.equal(sets.mock.callCount(), 1);
  assert.equal(paint, sets.mock.calls[0]!.result);
  assert.equal(paint.size, 2);
  assert.deepEqual(roles(paint, actual.output.text.length), {
    titles: 0,
    markers: 0,
    emphasis: 2,
  });
  assert.equal(prepareReorderScenePaint(actual), paint);
  assert.equal(sets.mock.callCount(), 1);
  assert.equal(readExactSource(f.state), source);
  assert.equal(undoDepth(f.state), 0);
});

test('moving an originally ordinary chip to the row opening creates the literal proposed heading', () => {
  const f = specimen('  shared, **One\nbody'),
    chip = f.capture.prepared.chips.find((chip) => chip.text === '**One')!.index;
  assert.equal(f.capture.prepared.scope.sceneContexts!.document.scenes.length, 0);
  const order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
    kind: 'row',
    row: 0,
    insertion: 0,
  });
  assert.ok(order);
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, '**One,   shared\nbody');
  assert.deepEqual(
    actual.scenes!.scenes.map((scene) => scene.marker.title),
    ['One,   shared'],
  );
  assert.deepEqual(roles(prepareReorderScenePaint(actual), actual.output.text.length), {
    titles: 1,
    markers: 1,
    emphasis: 0,
  });
  assert.equal(preview(f, order, chip).scenes, actual.scenes);
  assert.equal(readExactSource(f.state), '  shared, **One\nbody');
  assert.equal(undoDepth(f.state), 0);
});
test('moving a heading inline removes its Scene paint and restores ordinary rich syntax from the original compilation', (context) => {
  const f = specimen('**(One:2.00), shared\nbody'),
    chip = f.capture.prepared.chips.find((chip) => chip.text === '**(One:2.00)')!.index;
  const order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
    kind: 'row',
    row: 0,
    insertion: 1,
  });
  assert.ok(order);
  assert.deepEqual(roles(f.state.field(richProjection).all, f.state.doc.length), {
    titles: 1,
    markers: 1,
    emphasis: 0,
  });
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, 'shared, **(One:2.00)\nbody');
  assert.equal(actual.scenes!.scenes.length, 0);
  const sets = context.mock.method(Decoration, 'set'),
    paint = prepareReorderScenePaint(actual);
  assert.equal(sets.mock.callCount(), 1);
  assert.equal(paint, sets.mock.calls[0]!.result);
  assert.deepEqual(roles(paint, actual.output.text.length), {
    titles: 0,
    markers: 0,
    emphasis: 2,
  });
  assert.equal(prepareReorderScenePaint(actual), paint);
  assert.equal(sets.mock.callCount(), 1);
  assert.equal(readExactSource(f.state), '**(One:2.00), shared\nbody');
  assert.equal(undoDepth(f.state), 0);
});
test('raw captures retain no Scene paint preparation', () => {
  const f = reorderState('shared\n**One\nbody'),
    capture = captureReorder(f.state),
    actual = prepareReorderPreview(
      { capture, order: capture.prepared.initial, token: {}, activeChip: 0 },
      'complete',
    );
  assert.ok(actual);
  assert.equal(actual.scenes, null);
  assert.equal(prepareReorderScenePaint(actual).size, 0);
});

/** Literal multiline emphasis surrounds an independent middle heading; neither wrapper may survive suppression of its whole outer role. */
test('crossing-heading roles stay complete in native and canonical proposed paint', () => {
  const source = '(shared\n**One\nbody:2.00)',
    f = specimen(source),
    native = f.state.field(richProjection);
  assert.deepEqual(roles(native.all, source.length), {
    titles: 1,
    markers: 1,
    emphasis: 0,
  });
  assert.deepEqual(roles(native.sceneSyntax, source.length), {
    titles: 0,
    markers: 0,
    emphasis: 2,
  });
  const actual = preview(f, f.capture.prepared.initial, 0);
  assert.deepEqual(roles(prepareReorderScenePaint(actual), actual.output.text.length), {
    titles: 1,
    markers: 1,
    emphasis: 0,
  });
  assert.equal(readExactSource(f.state), source);
  assert.equal(undoDepth(f.state), 0);
});

/** Immutable donor reproduction demonstrates stale base bands; the deliberate port correction binds proposed bands to Core's newly classified scopes. */
test('an inline-moved heading transfers Scene parity to the remaining proposed Scene', () => {
  const f = specimen('common\n**One, shared\nbody\n**Two\nlast'),
    chip = f.capture.prepared.chips.find((chip) => chip.text === '**One')!.index;
  const order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
    kind: 'row',
    row: 1,
    insertion: 1,
  });
  assert.ok(order);
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, 'common\nshared, **One\nbody\n**Two\nlast');
  assert.deepEqual(
    actual.scenes!.scenes.map((scene) => scene.marker.title),
    ['Two'],
  );
  assert.deepEqual(prepareSceneBands(actual.scenes), [{ from: 28, to: 36 }]);
  assert.deepEqual(prepareSceneBands(f.capture.prepared.scope.sceneContexts!.document), [
    { from: 9, to: 26 },
  ]);
  assert.equal(readExactSource(f.state), 'common\n**One, shared\nbody\n**Two\nlast');
  assert.equal(undoDepth(f.state), 0);
});

/** Core's reopened shell occurrences are independent roles even when their original SourceToken is shared and a Scene heading lies between them. */
test('reopened emphasis occurrences on opposite sides of a heading retain both complete pairs', () => {
  const f = specimen('(first, last:2),\n**One\nbody', 8),
    chip = f.capture.active;
  const order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
    kind: 'row',
    row: 2,
    insertion: 1,
  });
  assert.ok(order);
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, '(first:2), \n**One\nbody, (last:2)');
  assert.equal(actual.output.origin.envelopes.length, 2);
  assert.deepEqual(roles(prepareReorderScenePaint(actual), actual.output.text.length), {
    titles: 1,
    markers: 1,
    emphasis: 4,
  });
  assert.equal(readExactSource(f.state), '(first, last:2),\n**One\nbody');
  assert.equal(undoDepth(f.state), 0);
});

/** A newly promoted middle heading owns its enclosing role even on remote unchanged hard-line fragments; their cached paint key must change. */
test('new middle headings suppress whole crossing roles and retire identical-origin cached wrappers', () => {
  const f = specimen('(first\nshared, **One\nafter:2.00)', 17),
    chip = f.capture.active;
  const original = preview(f, f.capture.prepared.initial, chip),
    originalPlan = prepareReorderWidgetPlan(original);
  const order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, chip, {
    kind: 'row',
    row: 1,
    insertion: 0,
  });
  assert.ok(order);
  const actual = preview(f, order, chip);
  assert.equal(actual.output.text, '(first\n**One, shared\nafter:2.00)');
  assert.deepEqual(roles(prepareReorderScenePaint(actual), actual.output.text.length), {
    titles: 1,
    markers: 1,
    emphasis: 0,
  });
  const first = (plan: ReturnType<typeof prepareReorderWidgetPlan>) => {
    const block = plan.blocks.find((block) => block.kind === 'chip');
    assert.ok(block?.kind === 'chip');
    const part = block.parts.find((part) => part.kind === 'line');
    assert.ok(part?.kind === 'line');
    return part;
  };
  const before = first(originalPlan),
    after = first(prepareReorderWidgetPlan(actual));
  assert.deepEqual(after.pieces, before.pieces);
  assert.deepEqual(
    [before.rowPaint?.sceneFill ?? false, after.rowPaint?.sceneFill ?? false],
    [false, false],
  );
  assert.equal(roles(before.decorations, before.to - before.from).emphasis, 1);
  assert.equal(roles(after.decorations, after.to - after.from).emphasis, 0);
  assert.notEqual(after.key, before.key);
  assert.equal(readExactSource(f.state), '(first\nshared, **One\nafter:2.00)');
  assert.equal(undoDepth(f.state), 0);
});
