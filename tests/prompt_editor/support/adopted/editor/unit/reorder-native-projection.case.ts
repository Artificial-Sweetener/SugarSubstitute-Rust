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
import { nativeReorderLineClass } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-targets.ts';
/** Actual Core/StateField issues characterize native target lifetime; literal DOM transport does not certify browser paint. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { nativeFixture } from './reorder-native-projection-fixture.ts';
import type { ViewUpdate } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import {
  borrowNativeReorderLayoutLease,
  currentNativeReorderLineOpacity,
  currentNativeReorderProjectionTargets,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-projection.ts';
import {
  currentReorderProjectionLease,
  currentReorderProjectionTargets,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-projection.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { readReorderBaseContext } from '../../../../../../frontend/sugarprompt/editor/src/reorder-base-context.ts';
import {
  currentReorderPaintTargets,
  ReorderWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import {
  installReorderPreview,
  reorderPreview,
  fallbackReorderPointerPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  setReorderMotionMask,
  releaseReorderMotionMask,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';

test('native initial pointer anchor reuses exact issued decorations, underlay and actual targets across semantic proposals', () => {
  const t = nativeFixture();
  try {
    const first = t.publish();
    assert.equal(first.native, true);
    assert.equal(first.decorations, t.before.decorations);
    assert.equal(first.outer, t.before.outer);
    assert.equal(first.outer.size, 0);
    const nativeLines: number[] = [];
    first.decorations.between(0, t.state.doc.length, (from, to, decoration) => {
      if (
        decoration.spec.class === nativeReorderLineClass(String(decoration.spec.nativeReorderValue))
      ) {
        assert.equal(from, to);
        nativeLines.push(from);
      }
    });
    assert.deepEqual(nativeLines, [0]);
    assert.equal(currentReorderPaintTargets(t.f.view), null);
    assert.equal(currentReorderProjectionTargets(t.f.view), null);
    const lease = currentReorderProjectionLease(t.f.view, first.value!);
    assert.ok(lease);
    assert.equal(lease(), true);
    const targets = currentReorderProjectionTargets(t.f.view)!;
    assert.deepEqual(
      targets.map((target) => target.node),
      t.f.chips,
    );
    const reads = t.sourceReads,
      allocations = t.f.allocations;
    for (let frame = 0; frame < 20; frame++) {
      assert.equal(lease(), true);
      assert.equal(currentReorderProjectionTargets(t.f.view), targets);
    }
    assert.equal(t.sourceReads, reads);
    assert.equal(t.f.allocations, allocations);
    const second = t.publish(t.pickup.order);
    assert.equal(second.pointer, first.pointer);
    assert.equal(lease(), false);
    const nextLease = currentReorderProjectionLease(t.f.view, second.value!);
    assert.ok(nextLease?.());
    assert.equal(currentReorderProjectionTargets(t.f.view), targets);
    assert.equal(t.sourceReads, reads);
    assert.equal(t.observers.length, 1);
    assert.equal(undoDepth(t.state), 0);
  } finally {
    t.owner.destroy();
  }
});

test('whole native coverage retains carriers until correct source-current canonical fallback publishes before mask retirement', () => {
  const t = nativeFixture();
  try {
    const first = t.publish(),
      lease = currentReorderProjectionLease(t.f.view, first.value!);
    assert.ok(lease);
    assert.equal(lease(), true);
    const issue = {};
    t.f.view.dispatch({
      effects: setReorderMotionMask.of({
        issue,
        scope: {},
        capture: t.pickup.capture,
        ids: [],
        coverage: 'value',
      }),
    });
    assert.equal(lease(), true);
    assert.equal(t.state.field(reorderPreview).outer, first.outer);
    assert.equal(
      fallbackReorderPointerPaint(t.f.view, first.pointer!, first.value!, () => true),
      true,
    );
    const fallback = t.state.field(reorderPreview);
    assert.equal(fallback.native, false);
    assert.equal(fallback.paint, fallback.value);
    assert.equal(fallback.paint!.order, t.moved);
    assert.equal(fallback.outer.size, 1);
    assert.equal(lease(), false);
    assert.equal(currentReorderProjectionTargets(t.f.view), null);
    assert.equal(t.observers[0]!.disconnected, true);
    t.f.view.dispatch({ effects: releaseReorderMotionMask.of(issue) });
    assert.equal(t.state.field(reorderPreview).outer.size, 0);
  } finally {
    t.owner.destroy();
  }
});

for (const reason of ['hold', 'source', 'geometry', 'viewport', 'mutation', 'destroy'] as const)
  test(`native projection retires exact issued targets on ${reason}`, () => {
    const t = nativeFixture();
    try {
      const first = t.publish(),
        lease = currentReorderProjectionLease(t.f.view, first.value!);
      assert.ok(lease);
      assert.equal(lease(), true);
      if (reason === 'hold') t.publish(t.moved, createReorderApplicationHold(t.pickup.order));
      else if (reason === 'source') t.f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      else if (reason === 'destroy') t.owner.destroy();
      else if (reason === 'mutation') t.observers[0]!.records.push({} as MutationRecord);
      else
        t.owner.update({
          docChanged: false,
          geometryChanged: reason === 'geometry',
          viewportChanged: reason === 'viewport',
        } as ViewUpdate);
      assert.equal(lease(), false);
      assert.equal(currentReorderProjectionTargets(t.f.view), null);
      assert.equal(t.observers[0]!.disconnected, true);
    } finally {
      t.owner.destroy();
    }
  });

test('native source interval admission rejects incomplete target identity and preserves raw position failures', () => {
  const t = nativeFixture();
  try {
    const first = t.publish();
    t.positions.set(t.f.chips[1] as unknown as HTMLElement, { from: 3, to: 3 });
    assert.equal(currentReorderProjectionLease(t.f.view, first.value!), null);
    assert.equal(t.observers.length, 0);
    const failure = new Error('native-position-fixture-failure');
    t.failPosition(failure);
    assert.throws(
      () => currentReorderProjectionLease(t.f.view, first.value!),
      (cause) => cause === failure,
    );
    assert.equal(currentReorderProjectionTargets(t.f.view), null);
  } finally {
    t.owner.destroy();
  }
});

test('native teardown clears registered identity before exposing original observer disposal failure', () => {
  const t = nativeFixture(),
    first = t.publish(),
    lease = currentReorderProjectionLease(t.f.view, first.value!);
  assert.ok(lease);
  assert.equal(lease(), true);
  const failure = new Error('native-disconnect-fixture-failure');
  t.failDisconnect(failure);
  assert.throws(
    () => t.owner.destroy(),
    (cause) => cause === failure,
  );
  assert.equal(lease(), false);
  assert.equal(currentReorderProjectionTargets(t.f.view), null);
  t.owner.destroy();
});

test('native admission fences reentrant newer issues without retiring their exact observer or target roster', () => {
  const t = nativeFixture();
  try {
    const first = t.publish();
    let newer: (() => boolean) | null = null;
    t.onPosition(() => {
      newer = currentReorderProjectionLease(t.f.view, first.value!);
    });
    assert.equal(currentReorderProjectionLease(t.f.view, first.value!), null);
    assert.ok(newer);
    assert.equal((newer as () => boolean)(), true);
    assert.equal(t.observers.length, 1);
    assert.equal(t.observers[0]!.disconnected, false);
    assert.equal(currentReorderProjectionTargets(t.f.view)!.length, 3);
  } finally {
    t.owner.destroy();
  }
});

for (const breaks of ['\n', '\n\n\n'])
  test(
    'split native hard-line fragments witness complete logical source across ' +
      breaks.length +
      ' native breaks',
    () => {
      const t = nativeFixture(`a, (b${breaks}c), d`);
      try {
        const first = t.publish(),
          extra = t.f.document.createElement('span');
        extra.dataset.reorderChip = '1';
        extra.append(t.f.text('c)'));
        t.f.line.append(extra);
        drawNativeReorderMetadata(t.f, t.state, [...t.f.chips, extra]);
        t.positions.set(t.f.chips[1] as unknown as HTMLElement, {
          from: 3,
          to: 5,
        });
        t.positions.set(extra as unknown as HTMLElement, {
          from: 5 + breaks.length,
          to: 7 + breaks.length,
        });
        t.positions.set(t.f.chips[2] as unknown as HTMLElement, {
          from: 9 + breaks.length,
          to: 10 + breaks.length,
        });
        const lease = currentReorderProjectionLease(t.f.view, first.value!);
        assert.ok(lease);
        assert.equal(lease(), true);
        assert.equal(
          currentReorderProjectionTargets(t.f.view)!.filter((target) => target.index === 1).length,
          2,
        );
        t.owner.update({
          docChanged: false,
          viewportChanged: true,
          geometryChanged: false,
        } as ViewUpdate);
        t.positions.set(extra as unknown as HTMLElement, {
          from: 6 + breaks.length,
          to: 7 + breaks.length,
        });
        assert.equal(currentReorderProjectionLease(t.f.view, first.value!), null);
      } finally {
        t.owner.destroy();
      }
    },
  );

test('native whole-value context admits actual affected hard-line union extent rather than only the first line', () => {
  const t = nativeFixture('a, (b\nc), d');
  try {
    const first = t.publish(),
      secondLine = t.f.document.createElement('div');
    secondLine.className = 'cm-line';
    Object.assign(secondLine.box, {
      left: 10,
      top: 40,
      width: 200,
      height: 20,
    });
    t.f.content.append(secondLine);
    t.f.view.domAtPos = (position: number) => ({
      node: (position >= 6 ? secondLine : t.f.line) as unknown as Node,
      offset: 0,
    });
    const context = readReorderBaseContext(t.f.view, first.value!);
    assert.ok(context);
    assert.equal(context.height, 40);
    assert.equal(context.width, 200);
    assert.equal(context.current(), true);
    secondLine.remove();
    assert.equal(context.current(), false);
  } finally {
    t.owner.destroy();
  }
});

test('ordinary native keyboard collision context keeps containing-line admission before a whole pointer Stage exists', () => {
  const t = nativeFixture('a, (b\nc), d');
  try {
    const keyboard = prepareReorderPreview(t.pickup, 'complete');
    assert.ok(keyboard);
    t.f.view.dispatch({ effects: installReorderPreview.of(keyboard) });
    assert.equal(t.state.field(reorderPreview).pointer, null);
    const context = readReorderBaseContext(t.f.view, keyboard);
    assert.ok(context);
    assert.equal(context.height, 20);
  } finally {
    t.owner.destroy();
  }
});

test('direct canonical Widget targets are borrowable under exact issued native pickup paint but reject a later different anchor', () => {
  const t = nativeFixture(),
    widget = new ReorderWidget(t.pickup, null);
  let root: HTMLElement | null = null;
  try {
    root = widget.toDOM(t.f.view);
    assert.ok(root.className.split(/\s+/u).includes('sp-reorder-pointer-underlay'));
    const passive = widget.mountPassive(t.f.view);
    try {
      assert.equal(
        passive.node.className.split(/\s+/u).includes('sp-reorder-pointer-underlay'),
        false,
      );
    } finally {
      passive.destroy();
    }
    t.f.line.replaceChildren(root as unknown as typeof t.f.line);
    const canonical = currentReorderPaintTargets(t.f.view);
    assert.ok(canonical);
    assert.equal(borrowNativeReorderLayoutLease(t.f.view, canonical), null);
    assert.equal(currentReorderProjectionTargets(t.f.view), canonical);
    const other = prepareReorderPreview({ ...t.pickup, activeChip: 1 }, 'pointer');
    assert.ok(other);
    t.f.view.dispatch({ effects: installReorderPreview.of(other) });
    assert.equal(currentReorderPaintTargets(t.f.view), canonical);
    assert.equal(currentReorderProjectionTargets(t.f.view), null);
  } finally {
    if (root) widget.destroy(root);
    t.owner.destroy();
  }
});

test('finite native issue retains actual pre-mask line opacity and emitted passive styles across warm covered proposals', () => {
  const t = nativeFixture();
  try {
    t.setOpacity('0.4');
    const first = t.publish(),
      lease = currentReorderProjectionLease(t.f.view, first.value!);
    assert.ok(lease);
    assert.equal(lease(), true);
    const initial = preparePassiveMotion(t.f.view, 0);
    assert.ok(initial);
    assert.match(initial.lineCss, /(?:^|;)opacity:0\.4;/);
    const reads = t.f.styleReads;
    assert.equal(
      currentNativeReorderLineOpacity(t.f.view, t.f.line as unknown as HTMLElement),
      '0.4',
    );
    assert.equal(t.f.styleReads, reads);
    t.f.view.dispatch({
      effects: setReorderMotionMask.of({
        issue: {},
        scope: {},
        capture: t.pickup.capture,
        ids: [],
        coverage: 'value',
      }),
    });
    assert.equal(t.f.document.defaultView.getComputedStyle(t.f.line).opacity, '0');
    const covered = preparePassiveMotion(t.f.view, 0);
    assert.ok(covered);
    assert.equal(covered.lineCss, initial.lineCss);
    const next = t.publish(),
      nextLease = currentReorderProjectionLease(t.f.view, next.value!);
    assert.ok(nextLease);
    assert.equal(nextLease(), true);
    const warm = preparePassiveMotion(t.f.view, 0);
    assert.ok(warm);
    assert.equal(warm.lineCss, initial.lineCss);
    t.owner.update({
      docChanged: false,
      viewportChanged: false,
      geometryChanged: true,
    } as ViewUpdate);
    assert.equal(currentNativeReorderProjectionTargets(t.f.view), null);
    const before = t.f.styleReads;
    assert.equal(currentReorderProjectionLease(t.f.view, next.value!), null);
    assert.equal(preparePassiveMotion(t.f.view, 0), null);
    assert.equal(t.f.styleReads, before);
    const pointer = t.state.field(reorderPreview).pointer!;
    assert.equal(
      fallbackReorderPointerPaint(t.f.view, pointer, next.value!, () => true),
      true,
    );
    assert.equal(t.state.field(reorderPreview).native, false);
    assert.equal(t.state.field(reorderPreview).paint!.order, t.moved);
    assert.equal(t.state.field(reorderPreview).outer.size, 1);
  } finally {
    t.owner.destroy();
  }
});
