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
/** Actual SourceLines adapts independent native row operands; browser proof owns real layout, layers and pixels. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type EditorState } from '@codemirror/state';
import { RectangleMarker, type EditorView } from '@codemirror/view';
import { SourceLines } from '../../../../../../frontend/sugarprompt/editor/src/source-lines.ts';
import { projectionState } from '../transaction/projection-state.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  reorderPreview,
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  mountCanonicalRowEdges,
  releaseCanonicalRowEdges,
} from '../../../../../../frontend/sugarprompt/editor/src/canonical-row-edges.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { attachReorderMotionStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';

/** Current semantic state is real; rectangles and requestMeasure are the finite native boundary only. */
function fixture(source = 'shared, peer\n**One\n[SEP|Face]\nbody') {
  const paint = motionPaintFixture();
  try {
    let state = initializeNativeStructure(projectionState(source));
    state = state.update({
      effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
    }).state;
    let composing = false;
    const jobs: unknown[] = [];
    const view = {
      get state() {
        return state;
      },
      get compositionStarted() {
        return composing;
      },
      set compositionStarted(value: boolean) {
        composing = value;
      },
      hasFocus: false,
      scaleX: 1.5,
      scaleY: 1.25,
      documentTop: 100,
      scrollDOM: {
        clientWidth: 120,
        clientHeight: 160,
        scrollLeft: 4,
        scrollTop: 10,
        getBoundingClientRect: () => ({ top: 80 }),
      },
      contentDOM: paint.content,
      get visibleRanges() {
        return [{ from: 0, to: state.doc.length }];
      },
      get viewportLineBlocks() {
        return [0, 20, 60, 80].slice(0, state.doc.lines).map((top, index) => {
          const line = state.doc.line(index + 1);
          return {
            from: line.from,
            to: line.to,
            top,
            height: index === 1 ? 40 : 20,
          };
        });
      },
      requestMeasure(request: unknown) {
        jobs.push(request);
      },
    } as unknown as EditorView;
    const owner = new SourceLines(view, () => 0, undefined);
    return {
      paint,
      view,
      owner,
      jobs,
      replaceState(value: EditorState) {
        state = value;
      },
      destroy() {
        const causes: unknown[] = [];
        for (const release of [() => owner.destroy(), () => paint.destroyLayer()])
          try {
            release();
          } catch (cause) {
            causes.push(cause);
          }
        if (causes.length === 1) throw causes[0];
        if (causes.length) throw new AggregateError(causes, 'scene-marker-fixture-retirement');
      },
    };
  } catch (primary) {
    try {
      paint.destroyLayer();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'scene-marker-fixture-acquisition');
    }
    throw primary;
  }
}
/** Every test primary precedes finite independent native retirement, including null/undefined causes. */
function proof(body: (f: ReturnType<typeof fixture>) => void, source?: string): void {
  const f = fixture(source),
    causes: unknown[] = [];
  try {
    body(f);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    f.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'scene-marker-proof-retirement');
}

test('ordinary Scene bands use exact supported block heights, independent scroll offsets and unequal axes', () =>
  proof((f) => {
    assert.deepEqual(
      f.owner.rowMarkers('scene').map((m) => [m.left, m.top, m.width, m.height]),
      [
        [6, 52.5, 180, 40],
        [6, 92.5, 180, 20],
        [6, 112.5, 180, 20],
      ],
    );
    assert.equal(
      f.owner
        .rowMarkers('scene')[0]!
        .eq(
          new RectangleMarker('sp-scene-fill-paint sp-scene-native-fill-paint', 6, 52.5, 180, 40),
        ),
      true,
    );
    assert.equal(f.owner.decorations.size, 3);
  }));

test('ordinary composition preserves current prepared bands but changed source never reuses stale bands', () =>
  proof((f) => {
    const before = f.owner.rowMarkers('scene');
    Object.assign(f.view, { compositionStarted: true });
    assert.deepEqual(f.owner.rowMarkers('scene'), before);
    assert.equal(f.owner.snapshot().status, 'composing');
    f.replaceState(f.view.state.update({ changes: { from: 0, insert: 'x' } }).state);
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
  }));

test('no Scene paint discovers no native geometry', () =>
  proof((f) => {
    Object.defineProperty(f.view, 'documentTop', {
      get() {
        throw new Error('empty-scene-document-read');
      },
    });
    Object.defineProperty(f.view, 'viewportLineBlocks', {
      get() {
        throw new Error('empty-scene-block-read');
      },
    });
    f.view.scrollDOM.getBoundingClientRect = () => {
      throw new Error('empty-scene-viewport-read');
    };
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
  }, 'plain, source'));

test('last native block discovery rejects a retired exact state and disposed owners cannot republish', () =>
  proof((f) => {
    Object.defineProperty(f.view, 'viewportLineBlocks', {
      get() {
        f.replaceState(f.view.state.update({ selection: { anchor: 1 } }).state);
        return [];
      },
    });
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
    f.owner.destroy();
    assert.deepEqual(f.owner.rowMarkers('scene'), []);
  }));

test('canonical row-edge delegates cannot publish when composition starts without an EditorState change', () =>
  proof((f) => {
    let state = f.view.state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state;
    const capture = captureReorder(state),
      order = moveReorderChip(capture.prepared, capture.prepared.initial, capture.active, {
        kind: 'row',
        row: 0,
        insertion: 1,
      });
    assert.ok(order);
    const preview = prepareReorderPreview(
      { token: {}, capture, activeChip: capture.active, order },
      'complete',
    );
    assert.ok(preview?.scenes);
    state = state.update({ effects: installReorderPreview.of(preview) }).state;
    assert.equal(state.field(reorderPreview).native, false);
    f.replaceState(state);
    const root = f.paint.document.createElement('span');
    Object.assign(root, {
      prepend: (node: typeof root) => root.insertBefore(node, root.childNodes[0] ?? null),
    });
    f.paint.content.append(root);
    const nativeRoot = root as unknown as HTMLElement,
      causes: unknown[] = [];
    try {
      assert.equal(
        mountCanonicalRowEdges(f.view, nativeRoot, { sceneFill: true, region: null }, () => true),
        true,
      );
      // The literal fixture exposes childNodes, matching the actual connected registered pair.
      const first = root.childNodes[0];
      assert.ok(first && first.nodeType === 1);
      first.getBoundingClientRect = () => {
        throw new Error('absent-regional-edge-read');
      };
      const viewportRead = f.view.scrollDOM.getBoundingClientRect;
      f.view.scrollDOM.getBoundingClientRect = () => {
        throw new Error('absent-regional-viewport-read');
      };
      assert.deepEqual(f.owner.rowMarkers('region'), []);
      f.view.scrollDOM.getBoundingClientRect = viewportRead;
      first.getBoundingClientRect = () => {
        Object.assign(f.view, { compositionStarted: true });
        return { top: 160, bottom: 160 } as DOMRect;
      };
      assert.deepEqual(f.owner.rowMarkers('scene'), []);
    } catch (cause) {
      causes.push(cause);
    }
    for (const release of [() => releaseCanonicalRowEdges(nativeRoot), () => root.remove()])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'canonical-composition-control-retirement');
  }));

test('actual native Scene tag preserves the existing external Stage refusal without a generated pseudo', () => {
  const f = motionPaintFixture(),
    causes: unknown[] = [];
  let node: ReturnType<typeof f.document.createElement> | null = null;
  let lease: ReturnType<typeof attachReorderMotionStage> = null;
  try {
    node = f.document.createElement('span');
    node.className = 'sp-reorder-motion-stage';
    node.style.width = '200px';
    node.style.height = '256px';
    f.line.className += ' sp-scene-fill sp-region-content';
    lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {});
    assert.ok(lease);
    f.drawLayer();
    assert.equal(lease.read(), 'waiting');
    assert.equal(f.layerMount.style.width, '0px');
    assert.equal(f.layerMount.style.height, '0px');
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => lease?.release(), () => node?.remove(), () => f.destroyLayer()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'scene-stage-refusal-retirement');
});
