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
/** Real Core/Widget/Stage/Presentation transport certifies contour/endpoint publication order, not browser raster parity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { wholeFixture, publishWhole } from './reorder-pointer-stage-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  observeReorderLandingPaint,
  readReorderLandingPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint-lease.ts';
import { ReorderLandingPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** The fixture independently supplies held endpoint [60,80]x[20,40]; inflation plus SVG stroke bounds are literal. */
function endpointContour(f: ReturnType<typeof wholeFixture>): void {
  const node = f.paint.document.body!.querySelector('.sp-reorder-landing');
  assert.ok(node);
  const svg = node.querySelector('svg');
  assert.ok(svg);
  assert.equal(svg.getAttribute('viewBox'), '55 17 30 26');
  assert.equal(
    node.querySelector('[data-reorder-landing-contour]')!.getAttribute('stroke'),
    'rgb(1, 2, 3)',
  );
}

test('whole Scene installs measured contour before endpoint metadata and its first clock write, without queued marker READ', () => {
  const f = wholeFixture();
  try {
    const source = readExactSource(f.state),
      selection = f.state.selection;
    f.presentation.mark(
      f.input,
      [{ kind: 'chip', index: 0, left: 20, right: 40, top: 20, bottom: 40 }],
      'black',
      { left: 0, right: 240, top: 0, bottom: 100 },
    );
    const pickup = f.paint.document.body!.querySelector('.sp-reorder-landing')!,
      create = f.paint.document.createElement.bind(f.paint.document);
    let endpoints = 0,
      frames = 0,
      refreshes = 0;
    f.paint.document.createElement = (name) => {
      const node = create(name);
      Object.defineProperty(node, 'dataset', {
        value: new Proxy(node.dataset, {
          set(target, key, value: string) {
            if (key === 'reorderMotionEndpoint') {
              endpointContour(f);
              assert.equal(f.frames.size, 0);
              endpoints++;
            }
            if (key === 'reorderMotionProgress') {
              endpointContour(f);
              assert.ok(node.dataset.reorderMotionEndpoint);
              frames++;
            }
            return Reflect.set(target, key, value);
          },
        }),
      });
      return node;
    };
    f.onRefresh((reason) => {
      if (reason === 'endpoint') {
        endpointContour(f);
        refreshes++;
      }
    });
    f.prepare();
    f.deliver();
    assert.deepEqual([endpoints, frames, refreshes], [1, 1, 1]);
    assert.equal(pickup.isConnected, false);
    assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test("pending C retains B's actual contour, then same-path C transfer survives B retirement", () => {
  const f = wholeFixture();
  try {
    const selection = f.state.selection;
    f.prepare();
    f.deliver();
    endpointContour(f);
    const node = f.paint.document.body!.querySelector('.sp-reorder-landing')!,
      path = node.querySelector('[data-reorder-landing-contour]')!,
      remove = node.remove.bind(node);
    publishWhole(f, 0);
    f.prepare();
    f.presentation.pending();
    assert.equal(f.paint.document.body!.querySelector('.sp-reorder-landing'), node);
    assert.equal(node.style.opacity, '0.52');
    endpointContour(f);
    node.remove = () => {
      throw new Error('older-contour-retirement-cannot-withdraw-new-endpoint');
    };
    try {
      f.deliver();
      assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
      assert.equal(f.paint.document.body!.querySelector('.sp-reorder-landing'), node);
      assert.equal(node.querySelector('[data-reorder-landing-contour]'), path);
      endpointContour(f);
    } finally {
      node.remove = remove;
    }
    f.presentation.cancel();
    assert.equal(node.isConnected, false);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test('cold Stage cannot borrow native pickup geometry without an issued display', () => {
  const f = wholeFixture();
  try {
    const preview = f.state.field(reorderPreview).value!;
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
    const original = f.view.scrollDOM.getBoundingClientRect;
    f.view.scrollDOM.getBoundingClientRect = () => {
      throw new Error('frozen-native-viewport-read');
    };
    try {
      assert.equal(
        readReorderLandingPaint(f.view, preview, null, () => {
          throw new Error('cold-stage-native-admission');
        }),
        null,
      );
      assert.equal(
        observeReorderLandingPaint(f.view, f.input, preview, () => true, null, [], null),
        null,
      );
    } finally {
      f.view.scrollDOM.getBoundingClientRect = original;
    }
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
  } finally {
    f.destroy();
  }
});

test('whole native endpoint outside the viewport admits its genuine Scene with explicit unavailable contour', () => {
  const f = wholeFixture();
  try {
    f.presentation.mark(
      f.input,
      [{ kind: 'chip', index: 0, left: 20, right: 40, top: 20, bottom: 40 }],
      'black',
      { left: 0, right: 240, top: 0, bottom: 100 },
    );
    const old = f.paint.document.body!.querySelector('.sp-reorder-landing')!,
      create = f.paint.document.createElement.bind(f.paint.document);
    f.paint.document.createElement = (name) => {
      const node = create(name),
        rect = node.getBoundingClientRect.bind(node);
      node.getBoundingClientRect = () => {
        const box = rect();
        let chip: typeof node | null = node;
        while (chip && chip.dataset.reorderChip === undefined) chip = chip.parent;
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        return chip?.dataset.reorderChip === '0' ? { ...box, left: 500, right: 520, x: 500 } : box;
      };
      return node;
    };
    f.prepare();
    f.deliver();
    assert.equal(old.isConnected, false);
    assert.equal(f.paint.document.body!.querySelector('.sp-reorder-landing'), null);
    assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
    assert.equal(f.state.field(reorderPreview).mask!.coverage, 'value');
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test("compatible C reentry at contour commit retains the already committed B endpoint until C's native READ", () => {
  const f = wholeFixture(),
    prepare = ReorderLandingPaint.prototype.prepare;
  try {
    f.prepare();
    f.deliver();
    const old = f.state.field(reorderPreview).mask!;
    let entered = false;
    ReorderLandingPaint.prototype.prepare = function (...args) {
      const publication = prepare.apply(this, args);
      return {
        ...publication,
        commit: (current) => {
          if (!entered) {
            entered = true;
            assert.notEqual(f.state.field(reorderPreview).mask!.issue, old.issue);
            publishWhole(f, 1);
            f.prepare();
            assert.equal(current(), true);
          }
          return publication.commit(current);
        },
      };
    };
    publishWhole(f, 0);
    f.prepare();
    f.deliver();
    assert.equal(entered, true);
    endpointContour(f);
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
    assert.ok(f.paint.editor.querySelector('[data-reorder-motion-endpoint]'));
    assert.ok(f.jobs.length > 0);
    ReorderLandingPaint.prototype.prepare = prepare;
    f.deliver();
    assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
    endpointContour(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    ReorderLandingPaint.prototype.prepare = prepare;
    f.destroy();
  }
});

test('issued display renews its own saved viewport and color without native pickup reads', () => {
  const f = wholeFixture();
  try {
    f.prepare();
    f.deliver();
    const viewport = f.view.scrollDOM.getBoundingClientRect,
      query = f.view.contentDOM.querySelector;
    f.view.scrollDOM.getBoundingClientRect = () => {
      throw new Error('display-cannot-reread-native-viewport');
    };
    f.view.contentDOM.querySelector = () => {
      throw new Error('display-cannot-query-native-held-color');
    };
    try {
      const display = f.presentation.displayPaint(f.input);
      assert.ok(display);
      const result = observeReorderLandingPaint(
        f.view,
        f.input,
        display.preview,
        display.current,
        null,
        display.geometry,
        display,
      );
      assert.ok(result);
      assert.equal(result.landingRects, display.rects);
      assert.deepEqual(result.viewport, display.viewport);
      assert.equal(result.color, 'rgb(1, 2, 3)');
      assert.equal(result.lease(), true);
    } finally {
      f.view.scrollDOM.getBoundingClientRect = viewport;
      f.view.contentDOM.querySelector = query;
    }
  } finally {
    f.destroy();
  }
});
