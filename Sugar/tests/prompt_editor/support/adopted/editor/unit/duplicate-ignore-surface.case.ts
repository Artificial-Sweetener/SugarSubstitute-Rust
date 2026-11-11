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
/** The actual mounted surface carries one native ignore field across baselines and input retirement, with no duplicate session mirror. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  EditorSelection,
  Transaction,
  type EditorState,
  type TransactionSpec,
} from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import {
  NativeSurface,
  type NativeViewFactory,
} from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import {
  duplicateIgnores,
  ignoreDuplicate,
  duplicateIgnoreKey,
  carryDuplicateIgnores,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-ignore.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { structureState } from '../transaction/document-structure-fixture.ts';
import {
  duplicateDiagnostics,
  prepareNativeDuplicates,
  installNativeDuplicates,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-diagnostics.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { duplicateMenuTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/duplicate-menu.ts';

/** Controlled platform transport preserves actual immutable state and invokes only scheduled native measurement callbacks. */
function fixture(initial: EditorState) {
  const body = {},
    root = { activeElement: null as unknown };
  const parent = { ownerDocument: { body } } as unknown as HTMLElement;
  const measurements: (() => void)[] = [];
  let destroyed = 0;
  const create: NativeViewFactory = (_parent, value) => {
    let state = value;
    const view = {
      root,
      dom: { contains: () => false },
      hasFocus: false,
      scrollDOM: { clientHeight: 40 },
      /** Native state is the sole source/history owner. */
      get state() {
        return state;
      },
      /** Host baseline publication uses CM's existing immutable state. */
      setState(next: EditorState) {
        state = next;
      },
      /** Controlled dispatch still constructs an actual native transaction. */
      dispatch(input: Transaction | TransactionSpec) {
        state = (input instanceof Transaction ? input : state.update(input)).state;
      },
      /** Controlled scroll snapshot carries no source or secondary scroll owner. */
      scrollSnapshot: () => ({}),
      /** Retain complete actual read/write requests for deterministic delivery. */
      requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
        measurements.push(() => {
          const read = request.read(view as unknown as EditorView);
          request.write?.(read, view as unknown as EditorView);
        });
      },
      /** Surface teardown destroys its exact view once. */
      destroy() {
        destroyed++;
      },
    };
    return view as unknown as EditorView;
  };
  const surface = new NativeSurface(parent, initial, () => {}, create);
  return {
    surface,
    /** Deliver only scheduled supported measurements and their publication microtasks. */
    async flush() {
      while (measurements.length) {
        measurements.shift()!();
        await Promise.resolve();
      }
    },
    destroyed: () => destroyed,
  };
}

test('same mounted baseline carries actual ignores without altering baseline document selection or reset history', () => {
  const initial = structureState('cat, cat', 'prompt', duplicateIgnores).state;
  const c = fixture(initial);
  try {
    const key = duplicateIgnoreKey(5, 8, 'cat');
    c.surface.view.dispatch({ effects: ignoreDuplicate.of(key) });
    const retained = c.surface.view.state.field(duplicateIgnores);
    const baseline = structureState('bird, bird', 'prompt', duplicateIgnores).state.update({
      selection: EditorSelection.single(9, 1),
    }).state;
    const actual = c.surface.setState(baseline);
    assert.equal(actual.field(duplicateIgnores), retained);
    assert.equal(actual.doc, baseline.doc);
    assert.equal(actual.selection, baseline.selection);
    assert.equal(undoDepth(actual), 0);
    assert.equal(actual.field(duplicateIgnores).length, 1);
    const headless = structureState('headless').state;
    assert.equal(c.surface.setState(headless).field(duplicateIgnores, false), undefined);
  } finally {
    c.surface.dispose();
  }
  assert.equal(c.destroyed(), 1);
});

test('actual input view retirement carries the CM field, while disposal and fresh remount start empty', async () => {
  const initial = structureState('cat, cat', 'prompt', [duplicateIgnores, loraMedia]).state;
  const c = fixture(initial);
  const key = duplicateIgnoreKey(5, 8, 'cat');
  c.surface.view.dispatch({ effects: ignoreDuplicate.of(key) });
  const previous = c.surface.view,
    retained = previous.state.field(duplicateIgnores);
  c.surface.retireInput(
    () => true,
    () => {},
  );
  await c.flush();
  assert.notEqual(c.surface.view, previous);
  assert.equal(c.surface.view.state.field(duplicateIgnores), retained);
  c.surface.dispose();
  assert.equal(c.destroyed(), 2);
  const remount = fixture(
    carryDuplicateIgnores(
      previous.state,
      structureState('cat, cat', 'prompt', duplicateIgnores).state,
    ),
  );
  assert.deepEqual(remount.surface.view.state.field(duplicateIgnores), []);
  assert.equal(remount.surface.view.state.doc.toString(), 'cat, cat');
  assert.equal(undoDepth(remount.surface.view.state), 0);
  remount.surface.dispose();
});

test('fresh actual surface restores complete prepared marks while same-surface baseline and platform retirement preserve suppression', async () => {
  let baseline = structureState('cat, cat, tail', 'prompt', [
    duplicateIgnores,
    duplicateDiagnostics,
    loraMedia,
  ]).state;
  const native = prepareNativeStructure(baseline);
  const prepared = prepareNativeDuplicates(baseline, native.nativeValues);
  baseline = baseline.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: native }),
      installNativeDuplicates.of(prepared),
    ],
  }).state;
  const target = duplicateMenuTargetAt(baseline, 5)!;
  baseline = baseline.update({
    effects: ignoreDuplicate.of(target.finding.ignoreKey),
  }).state;
  const suppressed = baseline.field(duplicateDiagnostics);
  assert.equal(suppressed.decorations.size, 0);
  assert.equal(suppressed.baseDecorations.size, 1);
  assert.equal(prepareNativeDuplicates(baseline, native.nativeValues), suppressed);
  const c = fixture(baseline);
  try {
    let state = c.surface.view.state;
    assert.deepEqual(state.field(duplicateIgnores), []);
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 1);
    assert.equal(state.field(duplicateDiagnostics).baseDecorations, suppressed.baseDecorations);
    assert.equal(state.field(duplicateDiagnostics).prepared, suppressed.prepared);
    assert.equal(state.doc, baseline.doc);
    assert.equal(state.selection, baseline.selection);
    assert.equal(undoDepth(state), 0);
    assert.ok(duplicateMenuTargetAt(state, 5));
    c.surface.view.dispatch({
      effects: ignoreDuplicate.of(target.finding.ignoreKey),
    });
    const ignored = c.surface.view.state.field(duplicateIgnores);
    assert.equal(c.surface.view.state.field(duplicateDiagnostics).decorations.size, 0);
    const visibleBaseline = structureState('cat, cat, tail', 'prompt', [
      duplicateIgnores,
      duplicateDiagnostics,
      loraMedia,
    ]).state;
    const visibleNative = prepareNativeStructure(visibleBaseline);
    const actualBaseline = visibleBaseline.update({
      effects: [
        installSemanticStructure.of({
          purpose: 'native',
          prepared: visibleNative,
        }),
        installNativeDuplicates.of(
          prepareNativeDuplicates(visibleBaseline, visibleNative.nativeValues),
        ),
      ],
    }).state;
    assert.equal(actualBaseline.field(duplicateDiagnostics).decorations.size, 1);
    state = c.surface.setState(actualBaseline);
    assert.equal(state.field(duplicateIgnores), ignored);
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
    assert.equal(state.field(duplicateDiagnostics).baseDecorations.size, 1);
    assert.equal(state.doc, actualBaseline.doc);
    assert.equal(state.selection, actualBaseline.selection);
    assert.equal(undoDepth(state), 0);
    c.surface.retireInput(
      () => true,
      () => {},
    );
    await c.flush();
    assert.equal(c.surface.view.state.field(duplicateIgnores), ignored);
    assert.equal(c.surface.view.state.field(duplicateDiagnostics).decorations.size, 0);
    assert.equal(c.surface.view.state.field(duplicateDiagnostics).baseDecorations.size, 1);
  } finally {
    c.surface.dispose();
  }
});
