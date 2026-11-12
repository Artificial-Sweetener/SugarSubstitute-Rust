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
/** Core destination topology and prepared native roles qualify complete canonical row presentation independently of DOM layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, StateEffect, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import {
  installReorderPreview,
  publishReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import { undoDepth, redoDepth } from '@codemirror/commands';
import {
  moveReorderChip,
  valueReorderLayout,
  emptyValueReorderPartitions,
  type PromptDocumentKind,
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
import { prepareReorderRowPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-row-paint.ts';
import {
  prepareReorderWidgetPlan,
  preparedReorderRowPaintAdmission,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';
import { preparedReorderSourceLines } from '../../../../../../frontend/sugarprompt/editor/src/source-line-origins.ts';
import { preparedRegionPaint } from '../../../../../../frontend/sugarprompt/editor/src/region-paint.ts';
import { canonicalReorderFragment } from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { projectionState } from '../transaction/projection-state.ts';

/** Supported native state preparation supplies the existing canonical scope, mappings and finite Core order owner. */
function specimen(source: string, position = 0, kind: PromptDocumentKind = 'prompt') {
  const initial = initializeNativeStructure(
      projectionState(source, new Compartment(), kind).update({
        selection: EditorSelection.cursor(position),
      }).state,
    ),
    state = initial.update({
      effects: installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
    }).state,
    capture = captureReorder(state, position);
  return { state, capture };
}
/** The exact Core order prepares all row operands once before any render or measurement. */
function preview(f: ReturnType<typeof specimen>, order = f.capture.prepared.initial) {
  const result = prepareReorderPreview(
    { capture: f.capture, order, activeChip: f.capture.active, token: {} },
    'complete',
  );
  assert.ok(result);
  return result;
}

/** Tests consume the same public Core topology outputs; expected roles remain independently fixed below. */
function rowPaint(
  actual: NonNullable<ReturnType<typeof prepareReorderPreview>>,
  lines: ReturnType<typeof preparedReorderSourceLines>,
) {
  return prepareReorderRowPaint(
    actual,
    lines,
    valueReorderLayout(actual.order),
    emptyValueReorderPartitions(actual.capture.prepared, actual.order),
  );
}
test('all canonical rows retain destination regional roles including an unfilled Scene heading and final body', () => {
  const source = 'shared\n**One\n[SEP|Face]\nfirst, second, third, fourth\n**Two\nlast',
    f = specimen(source, source.indexOf('first')),
    actual = preview(f),
    lines = preparedReorderSourceLines(f.capture, actual.output),
    paints = rowPaint(actual, lines),
    palette = preparedRegionPaint(f.capture.prepared.scope);
  assert.deepEqual(
    lines.map((line) => [
      paints.get(line)?.sceneFill ?? false,
      paints.get(line)?.region?.ordinal ?? null,
    ]),
    [
      [false, null],
      [true, null],
      [true, null],
      [true, 0],
      [false, 0],
      [false, 0],
    ],
  );
  for (const line of lines.slice(3)) assert.equal(paints.get(line)!.region, palette[0]);
  assert.equal(preparedRegionPaint(f.capture.prepared.scope), palette);
  const plan = prepareReorderWidgetPlan(actual);
  assert.equal(prepareReorderWidgetPlan(actual), plan);
  assert.equal(readExactSource(f.state), source);
  assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
});

test('a chip crossing a regional fence consumes its actual Core destination rather than copied source rail marks', () => {
  const source = 'shared\n[SEP|Face]\nfirst, second',
    f = specimen(source, source.indexOf('first')),
    order = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, f.capture.active, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
  assert.ok(order);
  assert.equal(f.capture.prepared.initial.partitions[f.capture.active], 1);
  assert.equal(order.partitions[f.capture.active], 0);
  const actual = preview(f, order),
    plan = prepareReorderWidgetPlan(actual),
    moved = plan.blocks.find((block) => block.kind === 'chip' && block.id === f.capture.active);
  assert.ok(moved?.kind === 'chip');
  assert.ok(
    moved.parts
      .filter((part) => part.kind === 'line')
      .every((part) => part.rowPaint?.region == null),
  );
  let inlineRails = 0;
  f.capture.projection!.decoded.between(0, source.length, (_from, _to, value) => {
    if (value.spec.attributes?.class === 'sp-region-content') inlineRails++;
  });
  assert.equal(inlineRails, 0);
  assert.equal(readExactSource(f.state), source);
});

test('actual empty regional destinations acquire full-row paint without Scene bands', () => {
  const source = '[SEP|One]\n\n[SEP|Two]\nend',
    f = specimen(source, source.indexOf('end')),
    actual = preview(f),
    lines = preparedReorderSourceLines(f.capture, actual.output),
    paint = rowPaint(actual, lines);
  assert.equal(lines.length, 4);
  assert.equal(paint.get(lines[1]!)?.region?.ordinal, 0);
  assert.equal(paint.get(lines[3]!)?.region?.ordinal, 1);
  assert.ok(Array.from(paint.values()).every((row) => row.sceneFill === false));
});

test('structured regional marks retain bounded inline ownership and acquire no prompt full-row resources', () => {
  const source = 'prompt\n"[SEP|Face]\nfirst, second"',
    f = specimen(source, source.indexOf('first'), 'wildcard-csv'),
    actual = preview(f),
    lines = preparedReorderSourceLines(f.capture, actual.output);
  assert.equal(rowPaint(actual, lines).size, 0);
  const range = f.capture.target.entry.semanticMapping!.logicalRange,
    projected = canonicalReorderFragment(f.state, f.state.field(richProjection).decorations, range);
  let rails = 0;
  projected.between(range.from, range.to, (_from, _to, value) => {
    if (value.spec.attributes?.class === 'sp-region-content') rails++;
  });
  assert.ok(rails > 0);
});

/** Cache-only admission classifies original prepared full-row roles without compiling inside Stage READ. */
test('canonical Stage admission refuses unprepared and regional-only plans while ordinary inline paint remains eligible', () => {
  for (const [source, expected] of [
    ['first, second', 'absent'],
    ['shared\n[SEP|Face]\nfirst, second', 'present'],
    ['shared\n**One\nfirst, second', 'present'],
  ] as const) {
    const actual = preview(specimen(source));
    assert.equal(preparedReorderRowPaintAdmission(actual), 'unprepared');
    const plan = prepareReorderWidgetPlan(actual);
    assert.equal(preparedReorderRowPaintAdmission(actual), expected);
    assert.equal(plan.nativeRowPaint, expected === 'present');
    assert.equal(prepareReorderWidgetPlan(actual), plan);
  }
});

/** Publication owns pure preparation before any native mount or READ; literal roles independently distinguish ordinary, Scene and regional resource admission. */
test('sticky canonical new holds retain full-row refusal and reuse the exact future paint plan before any widget mount', () => {
  for (const [source, expected] of [
    ['first, second, third', 'absent'],
    ['first, second, third\n**One\nbody', 'present'],
    ['first, second, third\n[SEP|Face]\nbody', 'present'],
  ] as const) {
    const f = specimen(source),
      capture = f.capture,
      token = {};
    let state = f.state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state;
    // Only actual CM State/transactions are needed; no native DOM, measurement or editor lifetime is simulated.
    const view = Object.create(EditorView.prototype) as EditorView;
    Object.defineProperties(view, {
      state: { get: () => state },
      hasFocus: { value: false },
    });
    Object.assign(view, {
      /** Apply genuine source-neutral transactions at the explicit external view boundary. */
      dispatch(spec: TransactionSpec): void {
        state = state.update(spec).state;
      },
    });
    const pickup = moveReorderChip(capture.prepared, capture.prepared.initial, capture.active, {
      kind: 'row',
      row: 0,
      insertion: 2,
    });
    assert.ok(pickup);
    assert.equal(
      publishReorderPreview(
        view,
        { capture, token, activeChip: capture.active, order: pickup },
        'complete',
        null,
        () => true,
      ),
      'published',
    );
    assert.equal(state.field(reorderPreview).native, false);
    assert.equal(
      preparedReorderRowPaintAdmission(state.field(reorderPreview).paint!),
      'unprepared',
    );
    const proposed = moveReorderChip(capture.prepared, pickup, capture.active, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
    assert.ok(proposed);
    const hold = createReorderApplicationHold(pickup);
    assert.equal(
      publishReorderPreview(
        view,
        { capture, token, activeChip: capture.active, order: proposed },
        'pointer',
        hold,
        () => true,
      ),
      'published',
    );
    const published = state.field(reorderPreview),
      anchor = published.pointer!.anchor;
    assert.equal(published.paint, anchor);
    assert.equal(anchor.order, pickup);
    assert.equal(preparedReorderRowPaintAdmission(anchor), expected);
    const plan = prepareReorderWidgetPlan(anchor);
    assert.equal(plan.nativeRowPaint, expected === 'present');
    assert.equal(prepareReorderWidgetPlan(anchor), plan);
    assert.equal(
      publishReorderPreview(
        view,
        { capture, token, activeChip: capture.active, order: pickup },
        'pointer',
        hold,
        () => true,
      ),
      'published',
    );
    assert.equal(state.field(reorderPreview).paint, anchor);
    assert.equal(prepareReorderWidgetPlan(anchor), plan);
    assert.equal(readExactSource(state), source);
    assert.equal(state.doc, f.state.doc);
    assert.equal(state.selection, f.state.selection);
    assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
    const ordinary = capture.projection!.ordinary,
      own = Object.getOwnPropertyDescriptor(ordinary, 'between'),
      original = ordinary.between,
      failures: unknown[] = [];
    let entered = false;
    Object.defineProperty(ordinary, 'between', {
      configurable: true,
      /** Classification may synchronously retire the native publication before its new anchor can dispatch. */
      value(...args: Parameters<typeof ordinary.between>) {
        if (!entered) {
          entered = true;
          state = state.update({
            effects: installReorderPreview.of(null),
          }).state;
        }
        return original.apply(ordinary, args);
      },
    });
    try {
      assert.equal(
        publishReorderPreview(
          view,
          { capture, token, activeChip: capture.active, order: proposed },
          'pointer',
          createReorderApplicationHold(pickup),
          () => true,
        ),
        'stale',
      );
      assert.equal(entered, true);
      assert.equal(state.field(reorderPreview).value, null);
      assert.equal(readExactSource(state), source);
      assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
    } catch (cause) {
      failures.push(cause);
    }
    try {
      if (own) Object.defineProperty(ordinary, 'between', own);
      else Reflect.deleteProperty(ordinary, 'between');
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'anchor-classification-proof-retirement');
  }
});
