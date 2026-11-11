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
/** Deferred native snapshots retain their own cleanup boundary after a newer primary press takes ownership. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { ReorderMeasurement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-measurement.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { key } from './reorder-session-fixture.ts';
import { paintFixture, drain } from './reorder-marker-fixture.ts';

for (const raw of [undefined, null, new Error('controlled-retired-grab-removal')] as const) {
  test(`retired snapshot removal preserves its raw cause without retiring the newer press (${raw === null ? 'null' : raw === undefined ? 'undefined' : 'error'})`, () => {
    const { f, nodes } = paintFixture();
    let measurement: ReorderMeasurement | null = null;
    let restore: (() => void) | null = null;
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      const preview = f.state.field(reorderPreview).value!;
      const first = {
        token: preview.token,
        capture: preview.capture,
        order: preview.order,
        held: preview.activeChip,
        hidden: null,
        pointer: 9,
        coordinates: { x: 15, y: 20 },
      };
      const second = { ...first, pointer: 10 };
      let current = first,
        failed = 0;
      measurement = new ReorderMeasurement(f.view, {
        current: (input) => input === current,
        collision: () => null,
        prepare: () => ({ kind: 'unavailable' }),
        restore: () => {},
        commit: () => {},
        failed: () => {
          failed++;
        },
      });
      measurement.begin(first);
      f.measure();
      const stale = f.tasks.shift();
      assert.ok(stale);
      const old = nodes.filter((node) => node.className === 'sp-reorder-proxy').at(-1);
      assert.ok(old);
      measurement.retarget();
      current = second;
      measurement.begin(second);
      f.measure();
      f.task();
      const live = nodes.filter((node) => node.className === 'sp-reorder-proxy').at(-1);
      assert.equal(live !== undefined && live !== old && !live.removed, true);
      const remove = old.remove;
      let removals = 0;
      old.remove = () => {
        removals++;
        remove.call(old);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      };
      restore = () => {
        old.remove = remove;
      };
      let caught = false,
        observed: unknown;
      try {
        stale();
      } catch (cause) {
        caught = true;
        observed = cause;
      }
      assert.equal(caught && observed === raw, true);
      assert.equal(removals, 1);
      assert.equal(failed, 0);
      assert.equal(live!.removed, false);
      assert.equal(f.captures.has(10), true);
      assert.equal(readExactSource(f.state) === 'a, b, c', true);
      assert.equal(undoDepth(f.state), 0);
      restore();
      restore = null;
      measurement.destroy();
      measurement = null;
      assert.equal(live!.removed, true);
      assert.equal(f.captures.size, 0);
    } finally {
      restore?.();
      try {
        measurement?.destroy();
      } finally {
        f.destroy();
      }
    }
  });
}
