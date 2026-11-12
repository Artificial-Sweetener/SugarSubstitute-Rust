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
/** Actual Stage/Slot readiness must survive native observer cleanup before a copied-layout result may publish. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';

test('successful disconnect detachment refuses actual Stage output and reconnect requires a fresh uncached READ', () => {
  const f = wholeFixture(),
    causes: unknown[] = [];
  let stage: ReorderPassiveStage | null = null;
  const window = f.view.dom.ownerDocument.defaultView!,
    descriptor = Object.getOwnPropertyDescriptor(window, 'MutationObserver')!,
    Native = window.MutationObserver;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    f.paint.admitLayer(stage);
    let selections = 0;
    const select = (geometry: Parameters<typeof reorderMotionTargets>[0]) => {
      selections++;
      return reorderMotionTargets(geometry, geometry, null);
    };
    const placement = child.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('native-placement-required');
    child.place(placement);
    const root = child.node.querySelector<HTMLElement>('[data-reorder-chip]')!;
    const native = root.getClientRects.bind(root);
    let reads = 0,
      disconnects = 0;
    root.getClientRects = () => {
      reads++;
      return native();
    };
    class Observer extends Native {
      /** Native cleanup succeeds but physically detaches the actual borrowed shell without changing its logical lease. */
      override disconnect(): void {
        super.disconnect();
        disconnects++;
        child!.node.remove();
      }
    }
    Object.defineProperty(window, 'MutationObserver', {
      configurable: true,
      value: Observer,
    });
    const source = readExactSource(f.state),
      selection = f.state.selection;
    assert.equal(child.read(select).kind, 'unavailable');
    assert.equal(disconnects, 1);
    assert.equal(child.node.isConnected, false);
    assert.equal(child.current(), false);
    assert.equal(selections, 1);
    assert.equal(reads, 1);
    Object.defineProperty(window, 'MutationObserver', descriptor);
    stage.node.append(child.node);
    assert.equal(child.current(), true);
    const fresh = child.read(select);
    assert.equal(fresh.kind, 'ready');
    assert.equal(reads, 2);
    assert.equal(selections, 2);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection === selection, true);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => Object.defineProperty(window, 'MutationObserver', descriptor),
    () => stage?.destroy(),
    () => f.destroy(),
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'slot-layout-actual-retirement-proof');
});
