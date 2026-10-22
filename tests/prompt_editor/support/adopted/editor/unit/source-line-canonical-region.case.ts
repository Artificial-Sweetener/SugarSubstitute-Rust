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
/** SourceLines owns regional-only canonical rail geometry, current hover color and finite retirement in its existing native layer. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, EditorSelection } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { SourceLines } from '../../../../../../frontend/sugarprompt/editor/src/source-lines.ts';
import { RegionRowMarker } from '../../../../../../frontend/sugarprompt/editor/src/region-row-marker.ts';
import { regionHover } from '../../../../../../frontend/sugarprompt/editor/src/region-hover.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { preparedRegionPaint } from '../../../../../../frontend/sugarprompt/editor/src/region-paint.ts';
import {
  mountCanonicalRowEdges,
  releaseCanonicalRowEdges,
} from '../../../../../../frontend/sugarprompt/editor/src/canonical-row-edges.ts';
import { projectionState } from '../transaction/projection-state.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Real source/projection/Core publication surrounds only literal controlled native rectangle boundaries. */
function fixture(hovered: boolean) {
  const paint = motionPaintFixture();
  let acquiredOwner: SourceLines | null = null,
    acquiredRoot: HTMLElement | null = null;
  try {
    const source = 'shared\n[SEP|Face]\nfirst, second',
      initial = initializeNativeStructure(
        projectionState(source).update({
          selection: EditorSelection.cursor(source.indexOf('first')),
        }).state,
      );
    let state = initial.update({
      effects: installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
    }).state;
    const target = Object.values(state.field(richProjection).regionHovers)[0]![0]!.target;
    state = state.update({
      effects: StateEffect.appendConfig.of([
        reorderPreviewExtension,
        regionHover.init(() =>
          hovered ? { source: state.field(editorFreshness).source, target } : null,
        ),
      ]),
    }).state;
    const capture = captureReorder(state),
      order = moveReorderChip(capture.prepared, capture.prepared.initial, capture.active, {
        kind: 'row',
        row: 1,
        insertion: 1,
      });
    assert.ok(order);
    const preview = prepareReorderPreview(
      { capture, order, activeChip: capture.active, token: {} },
      'complete',
    );
    assert.ok(preview);
    state = state.update({ effects: installReorderPreview.of(preview) }).state;
    const view = {
      get state() {
        return state;
      },
      compositionStarted: false,
      hasFocus: false,
      scaleX: 1.5,
      scaleY: 1.25,
      documentTop: 100,
      scrollDOM: {
        clientWidth: 120,
        clientHeight: 160,
        scrollLeft: 4,
        scrollTop: 10,
        getBoundingClientRect: () => ({ left: 20, top: 80 }),
      },
      contentDOM: paint.content,
      visibleRanges: [{ from: 0, to: state.doc.length }],
      viewportLineBlocks: [],
      /** Unit proof never executes asynchronous native work; the real plugin still owns its finite queued callback. */
      requestMeasure() {},
    } as unknown as EditorView;
    const owner = new SourceLines(view, () => 0, undefined);
    acquiredOwner = owner;
    const root = paint.document.createElement('span');
    acquiredRoot = root as unknown as HTMLElement;
    Object.assign(root, {
      prepend: (node: typeof root) => root.insertBefore(node, root.childNodes[0] ?? null),
    });
    paint.line.append(root);
    paint.line.getBoundingClientRect = () => ({
      left: 44,
      top: 100,
      right: 244,
      bottom: 200,
      width: 200,
      height: 100,
      x: 44,
      y: 100,
      toJSON: () => ({}),
    });
    const native = root as unknown as HTMLElement,
      region = preparedRegionPaint(capture.prepared.scope)[0]!;
    assert.equal(
      mountCanonicalRowEdges(view, native, { sceneFill: false, region }, () => true),
      true,
    );
    let edgeReads = 0;
    const top = root.childNodes[0],
      bottom = root.childNodes[1];
    assert.ok(top?.nodeType === 1 && bottom?.nodeType === 1);
    top.getBoundingClientRect = () => {
      edgeReads++;
      return { top: 160, bottom: 160 } as DOMRect;
    };
    bottom.getBoundingClientRect = () => {
      edgeReads++;
      return { top: 179, bottom: 179 } as DOMRect;
    };
    return {
      paint,
      view,
      owner,
      root: native,
      region,
      /** Exact native edge accesses distinguish an absent plane from a repeated current READ. */
      reads: () => edgeReads,
      /** Browser layout may change while EditorState identity remains the same. */
      moveEdges() {
        top.getBoundingClientRect = () => {
          edgeReads++;
          return { top: 180, bottom: 180 } as DOMRect;
        };
        bottom.getBoundingClientRect = () => {
          edgeReads++;
          return { top: 199, bottom: 199 } as DOMRect;
        };
      },
      /** State replacement invalidates the same captured READ without creating another editable source owner. */
      invalidate() {
        state = state.update({ selection: EditorSelection.cursor(0) }).state;
      },
    };
  } catch (primary) {
    const causes: unknown[] = [primary];
    for (const cleanup of [
      () => acquiredOwner?.destroy(),
      () => {
        if (acquiredRoot) releaseCanonicalRowEdges(acquiredRoot);
      },
      () => acquiredRoot?.remove(),
      () => paint.destroyLayer(),
    ])
      try {
        cleanup();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw primary;
    throw new AggregateError(causes, 'canonical-region-fixture-acquisition');
  }
} /** Body failures precede every original publication, row lease, native fixture and global-boundary restoration failure. */
function proof(hovered: boolean, body: (f: ReturnType<typeof fixture>) => void): void {
  const f = fixture(hovered),
    previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    causes: unknown[] = [];
  try {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: f.paint.document,
    });
    body(f);
  } catch (cause) {
    causes.push(cause);
  }
  for (const cleanup of [
    () => f.owner.destroy(),
    () => releaseCanonicalRowEdges(f.root),
    () => f.root.remove(),
    () => f.paint.destroyLayer(),
    () => {
      if (previous) Object.defineProperty(globalThis, 'document', previous);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ])
    try {
      cleanup();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'canonical-region-owner-proof-retirement');
}

for (const hovered of [false, true])
  test(`regional-only canonical rows share exact native unequal-axis plane and prepared ${hovered ? 'hover' : 'normal'} paint`, () =>
    proof(hovered, (f) => {
      const markers = f.owner.rowMarkers('region');
      assert.equal(markers.length, 1);
      const marker = markers[0];
      assert.ok(marker instanceof RegionRowMarker);
      assert.deepEqual(
        [marker.left, marker.top, marker.width, marker.height],
        hovered ? [22.875, 92.5, 5.25, 19] : [24, 92.5, 3, 19],
      );
      const node = marker.draw(),
        color = hovered ? f.region.hover : f.region.normal;
      assert.equal(node.style.backgroundColor, color);
      const repeated = f.owner.rowMarkers('region')[0]!;
      assert.equal(marker.eq(repeated), true);
      assert.equal(marker.update(node, repeated), true);
      assert.equal(node.style.backgroundColor, color);
      f.owner.destroy();
      assert.deepEqual(f.owner.rowMarkers('region'), []);
    }));

test('composition and state reentry cannot publish captured regional-only row ink', () =>
  proof(false, (f) => {
    Object.assign(f.view, { compositionStarted: true });
    assert.deepEqual(f.owner.rowMarkers('region'), []);
    Object.assign(f.view, { compositionStarted: false });
    f.paint.line.getBoundingClientRect = () => {
      f.invalidate();
      return { left: 44 } as DOMRect;
    };
    assert.deepEqual(f.owner.rowMarkers('region'), []);
  }));

/** Independent layer reads never borrow a previous callback's geometry; empty roles perform no native edge access. */
test('empty Scene plane and repeated regional READ preserve exact same-state geometry freshness', () =>
  proof(false, (f) => {
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
    assert.equal(f.reads(), 0);
    const first = f.owner.rowMarkers('region')[0]!;
    assert.equal(first.top, 92.5);
    assert.equal(f.reads(), 2);
    f.moveEdges();
    const next = f.owner.rowMarkers('region')[0]!;
    assert.equal(next.top, 112.5);
    assert.equal(f.reads(), 4);
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
    assert.equal(f.reads(), 4);
  }));
