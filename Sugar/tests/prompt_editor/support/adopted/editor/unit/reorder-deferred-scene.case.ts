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
/** Canonical preparation preserves exact currentness and native fallback across its finite read/write boundary. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
for (const retirement of [
  'new-proposal',
  'cancel',
  'selection',
  'dispose',
  'live-paint',
] as const) {
  test(`prepared scene ${retirement} retirement allocates no obsolete destination`, () => {
    const f = motionPresentationFixture();
    try {
      const allocations = f.paint.allocations,
        selection = f.state.selection;
      f.prepare();
      const read = f.jobs.shift();
      assert.ok(read);
      read();
      assert.equal(f.paint.allocations, allocations, 'scene-materialized-before-deferred-guard');
      const write = f.tasks.shift();
      assert.ok(write);
      if (retirement === 'new-proposal') {
        f.publish();
        f.prepare();
      } else if (retirement === 'cancel') f.presentation.cancel();
      else if (retirement === 'selection') f.dispatch({ selection: EditorSelection.range(4, 1) });
      else if (retirement === 'dispose') f.presentation.destroy();
      else f.widget.destroy(f.root);
      write();
      assert.equal(f.paint.allocations, allocations);
      assert.equal(readExactSource(f.state), 'a, b, c');
      if (retirement !== 'selection') assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.paint.document.head!.childNodes.length, 0);
      assert.equal(f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });
}

test('captured three-wrapper CSS reaches canonical WRITE without native style rediscovery', () => {
  const f = motionPresentationFixture();
  try {
    const baseline = f.paint.allocations;
    f.prepare();
    f.jobs.shift()!();
    assert.equal(f.paint.allocations, baseline);
    const reads = f.paint.styleReads,
      original = f.paint.document.defaultView.getComputedStyle;
    f.paint.document.defaultView.getComputedStyle = () => {
      throw new Error('style-rediscovery-in-write');
    };
    try {
      f.tasks.shift()!();
      assert.equal(f.paint.styleReads, reads);
      assert.ok(f.paint.allocations > baseline);
    } finally {
      f.paint.document.defaultView.getComputedStyle = original;
    }
    assert.equal(f.state.field(reorderPreview).mask, null);
    f.deliver();
    assert.ok(f.state.field(reorderPreview).mask);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
for (const primary of [null, undefined, new Error('controlled-canonical-mount')] as const)
  for (const cleanupFails of [false, true])
    test(`canonical mount failure retains raw primary ${String(primary)} and cleanup=${cleanupFails}`, () => {
      const f = motionPresentationFixture(),
        cleanup = new Error('controlled-canonical-cleanup');
      try {
        const create = f.paint.document.createElement.bind(f.paint.document);
        let removed = 0;
        f.paint.document.createElement = (name) => {
          const node = create(name),
            remove = node.remove.bind(node),
            append = node.append.bind(node);
          node.append = (...children) => {
            append(...children);
            if (
              node.className === 'sp-reorder-motion-stage' &&
              children.some(
                (child) => 'className' in child && child.className === 'sp-reorder-motion-paint',
              )
            )
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw primary;
          };
          node.remove = () => {
            remove();
            if (node.className === 'sp-reorder-motion-paint') {
              removed++;
              if (cleanupFails) throw cleanup;
            }
          };
          return node;
        };
        f.prepare();
        f.jobs.shift()!();
        let failed = false,
          actual: unknown;
        try {
          f.tasks.shift()!();
        } catch (cause) {
          failed = true;
          actual = cause;
        }
        assert.equal(failed, true);
        if (cleanupFails) {
          assert.ok(actual instanceof AggregateError);
          assert.deepEqual(actual.errors, [primary, cleanup]);
        } else assert.equal(actual, primary);
        assert.equal(removed, 1);
        assert.equal(f.state.field(reorderPreview).mask, null);
        assert.equal(f.frames.size, 0);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
      } finally {
        f.destroy();
      }
    });
test('unavailable complete canonical native output is reused under its exact live lease without allocation or rediscovery', () => {
  const f = motionPresentationFixture();
  try {
    const create = f.paint.document.createElement.bind(f.paint.document);
    f.paint.document.createElement = (name) => {
      const node = create(name),
        rect = node.getBoundingClientRect.bind(node);
      node.getBoundingClientRect = () => {
        const box = rect();
        let ancestor: typeof node | null = node;
        while (ancestor && ancestor.dataset.reorderChip === undefined) ancestor = ancestor.parent;
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        return ancestor ? { ...box, left: 400, right: 420, x: 400 } : box;
      };
      return node;
    };
    const selection = f.state.selection;
    f.prepare();
    f.deliver();
    assert.equal(f.state.field(reorderPreview).mask, null);
    const allocations = f.paint.allocations,
      reads = f.paint.styleReads;
    f.prepare();
    assert.equal(f.jobs.length, 0);
    assert.equal(f.paint.allocations, allocations);
    assert.equal(f.paint.styleReads, reads);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
