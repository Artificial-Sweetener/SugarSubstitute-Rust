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
import { drawNativeReorderMetadata } from './reorder-native-metadata-fixture.ts';
/** Initial native outer decorations retain actual origin geometry before any changed-order ReorderWidget is mounted. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  installReorderPreview,
  reorderPreviewExtension,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { currentReorderPaintTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderMotionPresentation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-presentation.ts';
import {
  nativeReorderViewportFootprint,
  admitNativeReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Install genuine native outer decorations without constructing a ReorderWidget or a duplicate editor. */
function nativeOriginFixture(rich: boolean, count = 3) {
  const f = motionPaintFixture(),
    source = rich
      ? 'a, <lora:fixture:0.70>, c'
      : count === 3
        ? 'a, b, c'
        : Array.from({ length: count }, (_, id) => String(id)).join(', ');
  let state = reorderState(source).state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  const capture = captureReorder(state),
    preview = prepareReorderPreview(
      { token: {}, capture, activeChip: 0, order: capture.prepared.initial },
      'complete',
    );
  assert.ok(preview);
  state = state.update({ effects: installReorderPreview.of(preview) }).state;
  assert.equal(state.field(reorderPreview).native, true);
  const marked: number[] = [];
  state.field(reorderPreview).decorations.between(0, state.doc.length, (_from, _to, decoration) => {
    const attributes: unknown = decoration.spec.attributes;
    if (attributes && typeof attributes === 'object' && 'data-reorder-chip' in attributes)
      marked.push(Number(attributes['data-reorder-chip']));
  });
  assert.deepEqual(
    marked,
    Array.from({ length: count }, (_, id) => id),
  );
  for (let id = 3; id < count; id++) {
    const chip = f.document.createElement('span');
    chip.dataset.reorderChip = String(id);
    Object.assign(chip.box, { left: 20, top: 20, width: 20, height: 20 });
    f.chips.push(chip);
    f.line.append(chip);
  }
  Object.defineProperty(f.view, 'state', { get: () => state });
  drawNativeReorderMetadata(f, state);
  assert.equal(currentReorderPaintTargets(f.view), null);
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document');
  let widget: LoraWidget | null = null,
    dom: HTMLElement | null = null;
  if (rich) {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: f.document,
    });
    widget = new LoraWidget(new SourceToken(), { name: 'fixture', resolution: 'missing' }, '0.70');
    dom = widget.toDOM(f.view);
    f.chips[1]!.childNodes.splice(0);
    f.chips[1]!.append(dom as unknown as typeof f.line);
    for (const node of f.chips[1]!.querySelectorAll()) Object.assign(node.box, f.chips[1]!.box);
  }
  const presentation = new ReorderMotionPresentation(f.view, {
    current: () => true,
    rects: () => [],
    refresh: () => {},
    observe: (action) => action(),
    /** Initial origin inspection is readonly and must never schedule preparation or allocate another scene. */
    schedule() {
      assert.fail('initial-origin-scheduled-work');
    },
  });
  return {
    f,
    state,
    source,
    presentation,
    /** The actual finite display and rich widget release while preserving native source/history ownership. */
    destroy() {
      presentation.destroy();
      if (widget && dom) widget.destroy(dom);
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  };
}

for (const rich of [false, true])
  test(`first keyboard native ${rich ? 'rich' : 'plain'} origin is observed before any replacement owner exists`, () => {
    const fixture = nativeOriginFixture(rich),
      { f, presentation } = fixture;
    try {
      const allocations = f.allocations,
        selection = fixture.state.selection,
        origin = presentation.before();
      assert.deepEqual([...new Set(origin.map((rect) => rect.index))], [0, 1, 2]);
      for (const id of [0, 1, 2]) {
        const actual = origin.find((rect) => rect.index === id);
        assert.ok(actual);
        assert.deepEqual(
          [actual.left, actual.top, actual.right, actual.bottom],
          [
            f.chips[id]!.box.left,
            f.chips[id]!.box.top,
            f.chips[id]!.box.left + 20,
            f.chips[id]!.box.top + 20,
          ],
        );
      }
      assert.equal(f.allocations, allocations);
      assert.equal(currentReorderPaintTargets(f.view), null);
      assert.equal(readExactSource(fixture.state), fixture.source);
      assert.equal(fixture.state.selection, selection);
      assert.equal(undoDepth(fixture.state), 0);
      assert.equal(redoDepth(fixture.state), 0);
    } finally {
      fixture.destroy();
    }
  });

test('native outer-mark origin uses layout admission even when the existing native line exceeds the bitmap-copy budget', () => {
  const fixture = nativeOriginFixture(true),
    { f, presentation } = fixture;
  try {
    for (let index = 0; index < 1200; index++) {
      const node = f.document.createElement('span');
      Object.assign(node.box, f.chips[0]!.box);
      f.chips[0]!.append(node);
    }
    assert.equal(admitNativeReorderPaint([f.line as unknown as HTMLElement]), null);
    const origin = presentation.before();
    assert.deepEqual([...new Set(origin.map((rect) => rect.index))], [0, 1, 2]);
    assert.ok(origin.length > 1024);
    assert.equal(readExactSource(fixture.state), fixture.source);
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    fixture.destroy();
  }
});

test('one offscreen native outer-mark fragment excludes the complete semantic ID while held lookup retains all actual fragments', () => {
  const fixture = nativeOriginFixture(false),
    { f, presentation } = fixture;
  try {
    const second = f.document.createElement('span');
    second.dataset.reorderChip = '0';
    second.setAttribute(
      'data-reorder-native-value',
      f.chips[0]!.getAttribute('data-reorder-native-value')!,
    );
    Object.assign(second.box, { left: 20, top: 10000, width: 20, height: 20 });
    f.line.append(second);
    assert.deepEqual([...new Set(presentation.before().map((rect) => rect.index))], [1, 2]);
    const held = nativeReorderViewportFootprint(f.view, 0);
    assert.ok(held);
    assert.equal(held.length, 2);
    assert.ok(held.some((rect) => rect.top === 10000));
  } finally {
    fixture.destroy();
  }
});

test('held native footprint admits only its actual roots before traversing 179 hostile peer subtrees', () => {
  const fixture = nativeOriginFixture(false),
    { f } = fixture;
  try {
    for (let id = 1; id < 180; id++) {
      let peer = f.chips[id];
      if (!peer) {
        peer = f.document.createElement('span');
        peer.dataset.reorderChip = String(id);
        f.line.append(peer);
      }
      Object.defineProperty(peer, 'childNodes', {
        get() {
          throw new Error('unselected-native-subtree');
        },
      });
    }
    // Public selector delivery is external: it returns the actual held root without traversing peer descendants.
    const original = f.view.contentDOM.querySelectorAll.bind(f.view.contentDOM);
    f.view.contentDOM.querySelectorAll = ((selector: string) =>
      selector === '[data-reorder-native-value="0"][data-reorder-chip="0"]'
        ? [f.chips[0]!]
        : selector === '.cm-line'
          ? [f.line]
          : original(selector)) as typeof f.view.contentDOM.querySelectorAll;
    const held = nativeReorderViewportFootprint(f.view, 0);
    assert.ok(held);
    assert.deepEqual(held, [
      {
        kind: 'chip',
        index: 0,
        left: f.chips[0]!.box.left,
        right: f.chips[0]!.box.left + f.chips[0]!.box.width,
        top: f.chips[0]!.box.top,
        bottom: f.chips[0]!.box.top + f.chips[0]!.box.height,
      },
    ]);
    assert.equal(readExactSource(fixture.state), fixture.source);
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    fixture.destroy();
  }
});

test('initial native origin retains130 actual complete groups independently of the64 animation cap', () => {
  const fixture = nativeOriginFixture(false, 130),
    { f, presentation } = fixture;
  try {
    const source = readExactSource(fixture.state),
      selection = fixture.state.selection;
    const origin = presentation.before();
    assert.equal(new Set(origin.map((rect) => rect.index)).size, 130);
    assert.deepEqual(origin, nativeReorderViewportFootprint(f.view));
    assert.equal(readExactSource(fixture.state), source);
    assert.equal(fixture.state.selection, selection);
    assert.equal(undoDepth(fixture.state), 0);
    assert.equal(redoDepth(fixture.state), 0);
    for (const chip of f.chips) {
      const box = chip.getBoundingClientRect();
      chip.getClientRects = () => Array.from({ length: 32 }, () => box);
    }
    assert.equal(nativeReorderViewportFootprint(f.view), null);
  } finally {
    fixture.destroy();
  }
});
