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
/** Native initial geometry borrows genuine CM admission and actual finite paint-node transport. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import {
  installReorderPreview,
  reorderPreviewExtension,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { readReorderLandingPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint-lease.ts';
import { currentReorderPaintLease } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Real captured source/selection and preview field issue permission; the existing paint transport owns only native DOM/style observations. */
function fixture() {
  const paint = motionPaintFixture();
  let state = reorderState('a, b, c').state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  const capture = captureReorder(state),
    token = {};
  Object.defineProperty(paint.view, 'state', { get: () => state });
  /** Publication uses the actual preview preparation and StateField native/canonical admission. */
  function publish(order = capture.prepared.initial) {
    const preview = prepareReorderPreview(
      { token, capture, activeChip: capture.active, order },
      'pointer',
    );
    assert.ok(preview);
    state = state.update({ effects: installReorderPreview.of(preview) }).state;
    drawNativeReorderMetadata(paint, state);
    return preview;
  }
  const preview = publish();
  return {
    ...paint,
    /** Forward the native transport observation instead of freezing its getter during object spread. */
    get styleReads() {
      return paint.styleReads;
    },
    capture,
    preview,
    publish,
    /** Source edits retire real capture permission through the existing CM field. */
    edit() {
      state = state.update({ changes: { from: 0, insert: 'x' } }).state;
    },
    /** Exact selection changes retire native geometry without changing any DOM coordinates. */
    select() {
      state = state.update({ selection: { anchor: 1 } }).state;
    },
    /** Retirement removes only this derived publication and its active paint attributes. */
    retire() {
      state = state.update({ effects: installReorderPreview.of(null) }).state;
    },
    /** Real editor-attribute facets remain derived from the current publication in both paint modes. */
    get activeAttribute() {
      return state
        .facet(EditorView.editorAttributes)
        .some(
          (attributes) =>
            typeof attributes === 'object' &&
            attributes !== null &&
            attributes['data-reorder-preview'] === 'true',
        );
    },
    /** Real current field observation is separate from controlled native geometry. */
    get native() {
      return state.field(reorderPreview).native;
    },
  };
}

test('initial native output issues a finite native lease without fabricating a canonical mount', () => {
  const f = fixture(),
    lease = readReorderLandingPaint(f.view, f.preview, null, (lease) => lease);
  assert.equal(f.native, true);
  assert.equal(currentReorderPaintLease(f.view, f.preview), null);
  assert.equal(lease?.(), true);
  const reads = f.styleReads;
  assert.equal(lease?.(), true);
  assert.equal(f.styleReads, reads);
  f.publish();
  assert.equal(lease?.(), false);
  assert.equal(
    readReorderLandingPaint(f.view, f.preview, null, (lease) => lease),
    null,
  );
});
for (const mutation of [
  'source',
  'selection',
  'ancestor',
  'held-style',
  'held-child',
  'held-identity',
  'disconnect',
  'canonical',
] as const)
  test(`native lease rejects actual ${mutation} supersession with unchanged original coordinates`, () => {
    const f = fixture(),
      lease = readReorderLandingPaint(f.view, f.preview, null, (lease) => lease);
    assert.equal(lease?.(), true);
    switch (mutation) {
      case 'source':
        f.edit();
        break;
      case 'selection':
        f.select();
        break;
      case 'ancestor':
        f.content.className += ' changed';
        break;
      case 'held-style':
        f.chips[0]!.style.setProperty('opacity', '0.5');
        break;
      case 'held-child':
        f.chips[0]!.append(f.text('x'));
        break;
      case 'held-identity':
        f.chips[0]!.setAttribute('data-reorder-chip', '1');
        break;
      case 'disconnect':
        f.chips[0]!.remove();
        break;
      case 'canonical': {
        const moved = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, 0, {
          kind: 'row',
          row: 0,
          insertion: 2,
        });
        assert.ok(moved);
        f.publish(moved);
        assert.equal(f.native, false);
        break;
      }
    }
    assert.equal(lease?.(), false);
  });

test('current landing output reuses its exact lease before native context and subtree preparation', () => {
  const f = fixture(),
    lease = readReorderLandingPaint(f.view, f.preview, null, (current) => current);
  assert.ok(lease);
  const actual = { rects: f.chips[0]!.getClientRects() },
    reads = f.styleReads;
  const output = readReorderLandingPaint(
    f.view,
    f.preview,
    { held: f.preview.activeChip, preview: f.preview, current: lease },
    () => actual,
  );
  assert.equal(output === actual, true);
  assert.equal(f.styleReads, reads);
  assert.equal(lease(), true);
});

test('native geometry cannot be certified by a later same-order publication', () => {
  const f = fixture();
  let reads = 0;
  const original = f.chips[0]!.getClientRects.bind(f.chips[0]!);
  f.chips[0]!.getClientRects = () => {
    reads++;
    f.publish();
    return original();
  };
  const output = readReorderLandingPaint(f.view, f.preview, null, (lease) => {
    assert.equal(lease(), true);
    const rects = f.chips[0]!.getClientRects();
    assert.equal(lease(), false);
    return rects;
  });
  assert.equal(reads, 1);
  assert.equal(output, null);
});
test('one actual publication derives active paint attributes for native and canonical output then retires them', () => {
  const f = fixture();
  assert.equal(f.native, true);
  assert.equal(f.activeAttribute, true);
  const moved = moveReorderChip(f.capture.prepared, f.capture.prepared.initial, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  });
  assert.ok(moved);
  f.publish(moved);
  assert.equal(f.native, false);
  assert.equal(f.activeAttribute, true);
  f.retire();
  assert.equal(f.activeAttribute, false);
});
for (const condition of ['disconnected', 'incompatible'] as const)
  test(`canonical ${condition} output rejects before any landing geometry despite retained authority`, (t) => {
    const f = motionPresentationFixture();
    t.after(() => f.destroy());
    const preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    const retained = currentReorderPaintLease(f.view, preview);
    assert.ok(retained);
    assert.equal(retained(), true);
    let requested = preview;
    if (condition === 'disconnected') {
      f.root.remove();
      const disconnected = currentReorderPaintLease(f.view, preview);
      assert.ok(disconnected);
      assert.equal(disconnected(), false);
    } else {
      const other = prepareReorderPreview(
        {
          token: f.input.token,
          capture: f.input.capture,
          activeChip: f.input.held,
          order: f.input.capture.prepared.initial,
        },
        'complete',
      );
      assert.ok(other);
      requested = other;
      assert.equal(currentReorderPaintLease(f.view, requested), null);
      assert.equal(retained(), true);
    }
    let reads = 0;
    const output = readReorderLandingPaint(
      f.view,
      requested,
      { held: f.input.held, preview, current: retained },
      () => {
        reads++;
        return f.root.getBoundingClientRect();
      },
    );
    assert.equal(output, null);
    assert.equal(reads, 0);
  });
test('another genuine native publication cannot supply retained landing authority', () => {
  const first = fixture(),
    next = fixture(),
    lease = readReorderLandingPaint(first.view, first.preview, null, (current) => current);
  assert.ok(lease);
  assert.equal(lease(), true);
  const reads = next.styleReads;
  const current = readReorderLandingPaint(
    next.view,
    next.preview,
    { held: first.preview.activeChip, preview: first.preview, current: lease },
    (issued) => issued,
  );
  assert.ok(current);
  assert.equal(current === lease, false);
  assert.equal(next.styleReads > reads, true);
  assert.equal(current(), true);
});
