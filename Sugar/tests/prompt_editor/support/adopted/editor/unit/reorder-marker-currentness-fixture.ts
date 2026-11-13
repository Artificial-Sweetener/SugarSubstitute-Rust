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
/** Real CM pointer publication/mounted widget owns every marker fragment; only caller coordinates and native phase delivery are controlled. */
import { readCurrentReorderGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-current-geometry.ts';
import assert from 'node:assert/strict';
import {
  hideReorderChip,
  dropReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { currentNativeReorder } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { ReorderPointerFeedback } from '../../../../../../frontend/sugarprompt/editor/src/reorder-pointer-feedback.ts';
import { ReorderPointerPlacement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-pointer-placement.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import { paintFixture } from './reorder-marker-fixture.ts';
import { key } from './reorder-session-fixture.ts';

/** Stationary geometry invokes real Placement.refresh; no alternate target/source rule is supplied. */
export function markerCurrentnessFixture() {
  const { f, nodes } = paintFixture();
  f.owner.keydown(key('Alt'));
  f.measure();
  f.task();
  const admitted = f.state.field(reorderPreview).value;
  assert.ok(admitted);
  const hidden = hideReorderChip(admitted.capture.prepared, admitted.order, admitted.activeChip);
  assert.ok(hidden);
  const input = {
    token: admitted.token,
    capture: admitted.capture,
    order: admitted.order,
    held: admitted.activeChip,
    hidden,
    pointer: 9,
    coordinates: { x: 15, y: 20 },
  };
  const preview = prepareReorderPreview(
    {
      token: input.token,
      capture: input.capture,
      order: input.order,
      activeChip: input.held,
    },
    'pointer',
  );
  assert.ok(preview);
  f.view.dispatch({ effects: installReorderPreview.of(preview) });
  f.draw();
  const feedback = new ReorderPointerFeedback(f.view);
  feedback.measureAppearance();
  const geometry = reorderGrabGeometry(
    [{ kind: 'chip', index: 0, left: 10, right: 30, top: 10, bottom: 30 }],
    0,
    input.coordinates,
  );
  assert.ok(geometry);
  feedback.begin(
    { node: f.view.dom.ownerDocument.createElement('span'), geometry },
    input.coordinates,
  );
  const landing: { pending?: boolean }[] = [];
  const placement = new ReorderPointerPlacement(
    f.view,
    {
      current: (value) => value === input && currentNativeReorder(f.state, input.capture),
      collision: () => {
        throw new Error('stationary-marker-collision-unexpected');
      },
      prepare: () => {
        throw new Error('stationary-marker-transform-unexpected');
      },
      restore: () => {
        throw new Error('stationary-marker-restore-unexpected');
      },
      commit: () => {
        throw new Error('stationary-marker-commit-unexpected');
      },
      failed: () => {
        throw new Error('stationary-marker-failure-unexpected');
      },
    },
    feedback,
    {
      displayPaint: () => null,
      observe: (action) => action(),
      schedule: (request) => f.view.requestMeasure(request),
      markerRects: (input) => {
        const rects = readCurrentReorderGeometry(f.view, {
          kind: 'marker',
          input,
        });
        if (!rects.length) throw new Error('stationary-marker-actual-paint-unavailable');
        return rects;
      },
      retain: () => {
        throw new Error('stationary-marker-retention-unexpected');
      },
      before: () => {
        throw new Error('stationary-marker-motion-origin-unexpected');
      },
      propose: () => {
        throw new Error('stationary-marker-proposal-unexpected');
      },
      pending: () => {},
      landing: (_input, _rects, _color, _viewport, pending) => {
        landing.push({ pending });
      },
    },
  );
  return {
    f,
    nodes,
    input,
    placement,
    landing,
    /** Actual queued native read/write job remains the observation boundary, never a private view handle. */
    issue() {
      placement.refresh(input);
      const request = f.jobs.shift();
      assert.ok(request);
      return request;
    },
    /** Publish a genuine adjacent Core destination and draw its actual updated widget before old marker acceptance. */
    replaceOrder() {
      const order = dropReorderChip(input.capture.prepared, input.hidden, input.held, {
        kind: 'row',
        row: 0,
        insertion: 2,
      });
      assert.ok(order);
      input.order = order;
      const preview = prepareReorderPreview(
        {
          token: input.token,
          capture: input.capture,
          order,
          activeChip: input.held,
        },
        'pointer',
      );
      assert.ok(preview);
      f.view.dispatch({ effects: installReorderPreview.of(preview) });
      f.draw();
    },
    /** Retire each actual geometry/paint owner before its borrowed CM fixture is destroyed. */
    destroy() {
      const causes: unknown[] = [];
      for (const release of [
        () => placement.destroy(),
        () => feedback.close(),
        () => f.destroy(),
      ]) {
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (causes.length === 1) throw causes[0];
      if (causes.length)
        throw new AggregateError(causes, 'marker-currentness-fixture-release-failed');
    },
  };
}
