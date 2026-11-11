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
/** Actual Slot/Pending/widget/layout composition owns one mutation fence per native checkpoint. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { observeReorderSlotLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import {
  ReorderWidget,
  type PassiveReorderPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';
import type { ReorderMotionScene } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';

/** Prepare the actual finite whole Stage and capture its genuine mounted painter without another renderer. */
function withRead(
  action: (input: {
    readonly f: ReturnType<typeof wholeFixture>;
    readonly stage: ReorderPassiveStage;
    readonly child: NonNullable<ReturnType<ReorderPassiveStage['mount']>>;
    readonly paint: PassiveReorderPaint;
    readonly prepared: NonNullable<ReturnType<typeof preparePassiveMotion>>;
    /** Register an independently attempted native/prototype restoration before changing its binding. */
    readonly restore: (release: () => void) => void;
  }) => void,
): void {
  const f = wholeFixture(),
    mount = ReorderWidget.prototype.mountPassive;
  let stage: ReorderPassiveStage | null = null;
  const causes: unknown[] = [],
    releases: (() => void)[] = [
      () => {
        ReorderWidget.prototype.mountPassive = mount;
      },
    ];
  const captured: { paint: PassiveReorderPaint | null } = { paint: null };
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    ReorderWidget.prototype.mountPassive = function (...args) {
      const paint = mount.apply(this, args);
      captured.paint = paint;
      return paint;
    };
    const child = stage.mount(prepared);
    ReorderWidget.prototype.mountPassive = mount;
    assert.ok(child);
    assert.ok(captured.paint);
    f.paint.admitLayer(stage);
    const placement = child.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('native-placement-required');
    child.place(placement);
    action({
      f,
      stage,
      child,
      paint: captured.paint,
      prepared,
      restore: (release) => releases.push(release),
    });
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [...releases.reverse(), () => stage?.destroy(), () => f.destroy()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'read-authority-proof-retirement');
}
/** Selection delegates to the actual production selector with the fixture's complete native output. */
function select(geometry: Parameters<typeof reorderMotionTargets>[0]) {
  return reorderMotionTargets(geometry, geometry, null);
}
/** Initial native peer boxes distinguish the moved peer from the unchanged passive destination control. */
function selectMovingPeers(geometry: Parameters<typeof reorderMotionTargets>[0]) {
  return reorderMotionTargets(
    [
      { kind: 'chip', index: 1, left: 60, right: 80, top: 20, bottom: 40 },
      { kind: 'chip', index: 2, left: 100, right: 120, top: 20, bottom: 40 },
    ],
    geometry,
    null,
  );
}
/** Compare every published numeric scene operand while checking finite roots separately. */
function output(scene: ReorderMotionScene) {
  return {
    geometry: scene.geometry,
    layout: scene.layout,
    targets: scene.targets,
    endpoint: scene.endpoint && {
      held: scene.endpoint.held,
      viewport: scene.endpoint.viewport,
      color: scene.endpoint.color,
      order: scene.endpoint.order.order,
      partitions: scene.endpoint.order.partitions,
    },
    chips: scene.chips.map((chip) => ({
      id: chip.id,
      rects: chip.rects,
      left: chip.left,
      top: chip.top,
      roots: chip.roots.map(({ scaleX, scaleY }) => ({ scaleX, scaleY })),
    })),
  };
}

test('actual complete whole output survives one Slot pre/post drain around every composed layout predicate', () => {
  let baseline: ReturnType<typeof output> | null = null;
  withRead(({ child, prepared }) => {
    const read = child.read(selectMovingPeers);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('baseline-ready-required');
    assert.equal(read.scene.endpoint?.order, prepared.preview.order);
    baseline = output(read.scene);
  });
  withRead(({ f, child, restore, prepared }) => {
    const window = f.view.dom.ownerDocument.defaultView!,
      Native = window.MutationObserver,
      observe = ReorderMountedLayout.prototype.observe,
      source = readExactSource(f.state),
      selection = f.state.selection;
    let insidePredicate = false,
      drains = 0,
      checkpoints = 0;
    /** Instrument the actual Slot subscription while retaining its native queue and cleanup. */
    class Observer extends Native {
      private slot = false;
      /** Only the actual original Slot observer is counted, leaving independent Stage observation intact. */
      override observe(node: Node, options?: MutationObserverInit): void {
        this.slot = node === child.node;
        super.observe(node, options);
      }
      /** A delegated bare authority must never recursively drain the actual Slot queue. */
      override takeRecords(): MutationRecord[] {
        if (this.slot) {
          if (insidePredicate) throw new Error('redundant-slot-drain-inside-composed-authority');
          drains++;
        }
        return super.takeRecords();
      }
    }
    restore(() => {
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value: Native,
      });
    });
    restore(() => {
      ReorderMountedLayout.prototype.observe = observe;
    });
    Object.defineProperty(window, 'MutationObserver', {
      configurable: true,
      value: Observer,
    });
    ReorderMountedLayout.prototype.observe = function (targets, authority, readTargets, coverage) {
      return observe.call(
        this,
        targets,
        {
          available: authority.available,
          guard: (predicate) => {
            const before = drains;
            const current = authority.guard(() => {
              insidePredicate = true;
              try {
                return predicate();
              } finally {
                insidePredicate = false;
              }
            });
            assert.equal(drains - before, 2);
            checkpoints++;
            return current;
          },
        },
        readTargets,
        coverage,
      );
    };
    {
      const read = child.read(selectMovingPeers);
      assert.equal(read.kind, 'ready');
      if (read.kind !== 'ready') assert.fail('composed-ready-required');
      assert.equal(read.scene.endpoint?.order, prepared.preview.order);
      assert.deepEqual(output(read.scene), baseline);
      assert.deepEqual(read.scene.targets, [
        {
          id: 1,
          kind: 'peer',
          from: { left: 60, top: 20 },
          to: { left: 20, top: 20 },
        },
      ]);
      assert.equal(
        read.scene.targets.some((target) => target.id === 2),
        false,
      );
      assert.ok(checkpoints > 0);
      // Each of three literal chips contributes its root and canonical text-span fragment.
      assert.equal(read.scene.geometry.length, 6);
      assert.equal(
        new Set(read.scene.chips.flatMap((chip) => chip.roots.map(({ node }) => node))).size,
        1,
      );
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state) + redoDepth(f.state), 0);
    }
  });
});

for (const hostile of ['mutation', 'state', 'source', 'same-preview-issue', 'retire'] as const)
  test(`the final mounted connection getter rejects ${hostile} before any later native geometry`, () => {
    withRead(({ f, child, paint, prepared, restore }) => {
      const observe = ReorderMountedLayout.prototype.observe,
        descriptor = Object.getOwnPropertyDescriptor(paint.node, 'isConnected'),
        source = readExactSource(f.state),
        selection = f.state.selection;
      let armed = false,
        connections = 0,
        fired = false,
        laterGeometry = 0;
      for (const target of paint.targets()) {
        const bounds = target.node.getBoundingClientRect,
          fragments = target.node.getClientRects;
        target.node.getBoundingClientRect = () => {
          if (fired) laterGeometry++;
          return bounds.call(target.node);
        };
        target.node.getClientRects = () => {
          if (fired) laterGeometry++;
          return fragments.call(target.node);
        };
      }
      restore(() => {
        ReorderMountedLayout.prototype.observe = observe;
      });
      restore(() => {
        if (descriptor) Object.defineProperty(paint.node, 'isConnected', descriptor);
        else Reflect.deleteProperty(paint.node, 'isConnected');
      });
      Object.defineProperty(paint.node, 'isConnected', {
        configurable: true,
        get() {
          if (armed && !fired && ++connections === 2) {
            fired = true;
            if (hostile === 'mutation') paint.node.setAttribute('data-hostile', '1');
            else if (hostile === 'state') f.view.dispatch({ selection: { anchor: 1 } });
            else if (hostile === 'source') f.view.dispatch({ changes: { from: 0, insert: 'x' } });
            else if (hostile === 'same-preview-issue')
              assert.equal(paint.reconcile(prepared.preview), 'ready');
            else child.destroy();
          }
          return true;
        },
      });
      ReorderMountedLayout.prototype.observe = function (
        targets,
        authority,
        readTargets,
        coverage,
      ) {
        return observe.call(
          this,
          targets,
          {
            available: authority.available,
            guard: (predicate) =>
              authority.guard(() => {
                armed = true;
                connections = 0;
                try {
                  return predicate();
                } finally {
                  armed = false;
                }
              }),
          },
          readTargets,
          coverage,
        );
      };
      {
        assert.equal(
          child.read(() => assert.fail('revoked-connection-reached-selection')).kind,
          'unavailable',
        );
        assert.equal(fired, true);
        assert.equal(laterGeometry, 0);
        assert.equal(readExactSource(f.state), hostile === 'source' ? `x${source}` : source);
        if (hostile !== 'source' && hostile !== 'state') assert.equal(f.state.selection, selection);
        assert.equal(undoDepth(f.state), hostile === 'source' ? 1 : 0);
        assert.equal(redoDepth(f.state), 0);
      }
    });
  });

for (const hostile of [
  'layout-retire',
  'disconnect',
  'state',
  'same-preview-issue',
  'slot-retire',
] as const)
  test(`terminal native queue delegation rejects ${hostile} before the next geometry getter`, () => {
    withRead(({ f, child, paint, prepared, restore }) => {
      const window = f.view.dom.ownerDocument.defaultView!,
        Native = window.MutationObserver,
        observe = ReorderMountedLayout.prototype.observe,
        source = readExactSource(f.state);
      let terminal: (() => void) | null = null,
        drains = 0,
        fired = false,
        laterGeometry = 0;
      for (const target of paint.targets()) {
        const bounds = target.node.getBoundingClientRect;
        target.node.getBoundingClientRect = () => {
          if (fired) laterGeometry++;
          return bounds.call(target.node);
        };
      }
      /** Reenter the actual Slot subscription only at its terminal queue delegation. */
      class Observer extends Native {
        private slot = false;
        /** The controlled reentry belongs to the same actual Slot observer used by the geometry request. */
        override observe(node: Node, options?: MutationObserverInit): void {
          this.slot = node === child.node;
          super.observe(node, options);
        }
        /** Delegate first, then reenter exactly at the second queue read of the first layout checkpoint. */
        override takeRecords(): MutationRecord[] {
          const records = super.takeRecords();
          if (this.slot && terminal && !fired && ++drains === 2) {
            fired = true;
            terminal();
          }
          return records;
        }
      }
      restore(() => {
        Object.defineProperty(window, 'MutationObserver', {
          configurable: true,
          value: Native,
        });
      });
      restore(() => {
        ReorderMountedLayout.prototype.observe = observe;
      });
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value: Observer,
      });
      ReorderMountedLayout.prototype.observe = function (
        targets,
        authority,
        readTargets,
        coverage,
      ) {
        terminal = () => {
          if (hostile === 'layout-retire') this.destroy();
          else if (hostile === 'disconnect') paint.node.remove();
          else if (hostile === 'state') f.view.dispatch({ selection: { anchor: 1 } });
          else if (hostile === 'same-preview-issue')
            assert.equal(paint.reconcile(prepared.preview), 'ready');
          else child.destroy();
        };
        try {
          return observe.call(this, targets, authority, readTargets, coverage);
        } finally {
          terminal = null;
        }
      };
      {
        assert.equal(
          child.read(() => assert.fail('terminal-queue-reentry-reached-selection')).kind,
          'unavailable',
        );
        assert.equal(fired, true);
        assert.equal(laterGeometry, 0);
        assert.equal(readExactSource(f.state), source);
        assert.equal(undoDepth(f.state) + redoDepth(f.state), 0);
      }
    });
  });

test('terminal native queue delegation revokes topology-only reservation before shell discovery or action', () => {
  withRead(({ f, child, restore }) => {
    const window = f.view.dom.ownerDocument.defaultView!,
      Native = window.MutationObserver;
    let reserved = true,
      drains = 0,
      disconnected = 0;
    /** Instrument the actual Slot subscription while retaining its native queue and cleanup. */
    class Observer extends Native {
      /** Revoke the genuine caller-owned reservation during the original Slot's terminal queue read. */
      override takeRecords(): MutationRecord[] {
        const records = super.takeRecords();
        if (++drains === 2) reserved = false;
        return records;
      }
      /** Observe actual native cleanup independently of reservation rejection. */
      override disconnect(): void {
        super.disconnect();
        disconnected++;
      }
    }
    restore(() => {
      Object.defineProperty(window, 'MutationObserver', {
        configurable: true,
        value: Native,
      });
    });
    Object.defineProperty(window, 'MutationObserver', {
      configurable: true,
      value: Observer,
    });
    {
      assert.equal(
        observeReorderSlotLayout(
          child.node,
          () => reserved,
          () => assert.fail('revoked-reservation-entered-topology-action'),
        ),
        null,
      );
      assert.equal(drains, 2);
      assert.equal(disconnected, 1);
      assert.equal(reserved, false);
    }
  });
});
