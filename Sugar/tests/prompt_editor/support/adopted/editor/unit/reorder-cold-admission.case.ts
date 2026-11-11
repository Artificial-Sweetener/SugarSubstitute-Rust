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
/** Actual Session/Measurement callbacks qualify cold publication phases; controlled native transports do not certify browser frame cost. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { paintFixture, pointerTransferFixture, drain } from './reorder-marker-fixture.ts';
import { pointer, key } from './reorder-session-fixture.ts';
import {
  reorderPreview,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { ReorderMeasurement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-measurement.ts';

test('threshold coalesces cold publication outside the pointer callback and preserves the original hotspot at latest coordinates', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer(1, 14, 12));
    drain(f);
    const initial = f.state.field(reorderPreview).value,
      selection = f.state.selection;
    let publications = 0;
    f.observe((tx) => {
      if (tx.effects.some((effect) => effect.is(installReorderPreview))) publications++;
    });
    f.owner.pointermove(pointer(1, 14, 22));
    f.owner.pointermove(pointer(1, 23, 63));
    assert.equal(publications, 0);
    assert.equal(f.state.field(reorderPreview).value, initial);
    assert.equal(f.jobs.length, 1);
    assert.equal(
      nodes.filter((node) => node.className.includes('sp-reorder-drag-surface')).length,
      0,
    );
    f.measure();
    assert.equal(publications, 0);
    f.task();
    assert.equal(publications, 1);
    assert.equal(f.state.field(reorderPreview).value?.presentation, 'pointer');
    const surface = nodes.find((node) => node.className.includes('sp-reorder-drag-surface'));
    assert.ok(surface);
    assert.equal(surface.style.transform, 'translate(61px, 19px)');
    assert.equal(surface.style.left, '0px');
    assert.equal(
      nodes.filter((node) => node.className.includes('sp-reorder-drag-surface')).length,
      1,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    drain(f);
  } finally {
    f.destroy();
  }
});

for (const release of ['pointerup', 'Alt'] as const)
  test(`pending cold ${release} retains the final destination and publishes one genuine history edit`, () => {
    const { f } = paintFixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      f.owner.pointermove(pointer(1, 20, 55));
      if (release === 'pointerup') f.owner.pointerup(pointer(1, 20, 55));
      else f.owner.keyup(key('Alt'));
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      drain(f);
      assert.equal(readExactSource(f.state), 'b, c, a');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

for (const retirement of ['Escape', 'source'] as const)
  test(`queued cold publication rejects ${retirement} retirement after its read`, () => {
    const { f, nodes } = paintFixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      f.owner.pointermove(pointer(1, 20, 55));
      f.measure();
      if (retirement === 'Escape') f.owner.keydown(key('Escape'));
      else f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      drain(f);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(
        nodes.some((node) => node.className.includes('sp-reorder-drag-surface') && !node.removed),
        false,
      );
      assert.equal(readExactSource(f.state), retirement === 'source' ? 'xa, b, c' : 'a, b, c');
      assert.equal(undoDepth(f.state), retirement === 'source' ? 1 : 0);
      assert.equal(f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

test('old admission cannot clear or publish over a newer same-session primary hold', () => {
  const c = pointerTransferFixture(),
    f = c.f;
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    f.owner.pointermove(pointer(1, 20, 55));
    f.measure();
    const old = f.tasks.shift();
    assert.ok(old);
    f.owner.pointerdown(c.down(1, 9, 20, 45));
    drain(f);
    const preview = f.state.field(reorderPreview).value,
      root = f.paintedRoot;
    assert.ok(preview);
    assert.equal(preview.activeChip, 1);
    old();
    drain(f);
    assert.equal(f.state.field(reorderPreview).value, preview);
    assert.equal(f.paintedRoot, root);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(
      c.nodes.some((node) => node.className.includes('sp-reorder-drag-surface') && !node.removed),
      false,
    );
  } finally {
    c.destroy();
  }
});

test('geometry refresh replaces the exact queued admission without reviving its stale write', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    let publications = 0;
    f.observe((tx) => {
      if (tx.effects.some((effect) => effect.is(installReorderPreview))) publications++;
    });
    f.owner.pointermove(pointer(1, 20, 55));
    f.measure();
    const old = f.tasks.shift();
    assert.ok(old);
    f.owner.refreshGeometry();
    assert.equal(f.jobs.length, 1);
    old();
    assert.equal(publications, 0);
    f.measure();
    f.task();
    assert.equal(publications, 1);
    assert.equal(
      nodes.filter((node) => node.className.includes('sp-reorder-drag-surface')).length,
      1,
    );
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('delegated unavailable admission retries only on real intent and retires an unavailable released drop', () => {
  const { f, nodes } = paintFixture();
  let measurement: ReorderMeasurement | null = null;
  const causes: unknown[] = [];
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    const input = {
      token: preview.token,
      capture: preview.capture,
      order: preview.order,
      held: preview.activeChip,
      hidden: null,
      pointer: 9,
      coordinates: { x: 15, y: 20 },
    };
    let available = true,
      collisions = 0,
      failures = 0;
    measurement = new ReorderMeasurement(f.view, {
      /** Session currentness is the only controlled external denial; Core/drop rules are not reproduced. */
      current: (value) => value === input && available,
      /** Expected unavailable collision is supplied at its actual callback boundary. */
      collision: () => {
        collisions++;
        return null;
      },
      prepare: () => {
        throw new Error('unavailable-placement-unexpected');
      },
      restore: () => {
        throw new Error('unavailable-restore-unexpected');
      },
      commit: () => {
        throw new Error('unavailable-commit-unexpected');
      },
      /** Actual resource retirement settles an unavailable released intent. */
      failed: () => {
        failures++;
        measurement!.cancel();
      },
    });
    measurement.begin(input);
    drain(f);
    input.coordinates = { x: 25, y: 20 };
    measurement.motion(input, false);
    available = false;
    f.measure();
    available = true;
    f.task();
    assert.equal(collisions, 0);
    measurement.motion(input, false);
    drain(f);
    assert.equal(collisions, 1);
    assert.equal(failures, 0);
    measurement.motion(input, false);
    drain(f);
    assert.equal(collisions, 2);
    measurement.motion(input, true);
    drain(f);
    assert.equal(collisions, 3);
    assert.equal(failures, 1);
    assert.equal(
      nodes.some((node) => node.className.includes('sp-reorder-drag-surface') && !node.removed),
      false,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.captures.size + f.frames.size, 0);
  } catch (cause) {
    causes.push(cause);
  } finally {
    for (const release of [() => measurement?.destroy(), () => f.destroy()])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'cold-unavailable-proof-failed');
});
