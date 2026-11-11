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
/** Real Session/collision requests consume continuous pointer intent between controlled native phases; browser60Hz cost remains native proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { fixture, pointer, key } from './reorder-session-fixture.ts';
import { drain, pointerTransferFixture } from './reorder-marker-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Observe the actual connected canonical collision resource, never private state or a fabricated geometry result. */
function root(f: ReturnType<typeof fixture>): HTMLElement {
  const node = Array.from(f.view.dom.ownerDocument.body.childNodes).find(
    (node) =>
      node.nodeType === 1 &&
      (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
  );
  assert.ok(node);
  return node as HTMLElement;
}
/** Deliver actual admission and mount WRITE, retaining its real follow-up task for explicit phase assertions. */
function mount(f: ReturnType<typeof fixture>): void {
  f.owner.pointerdown(pointer());
  drain(f);
  f.owner.pointermove(pointer(1, 20, 25));
  f.measure();
  f.task();
  f.measure();
}

test('continuous moves retain one actual canonical base through separate calibration and physical reads', () => {
  const f = fixture();
  try {
    mount(f);
    const base = root(f),
      tree = base.childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1],
      selection = f.state.selection;
    assert.equal(f.jobs.length, 0);
    for (const x of [35, 45, 55]) f.owner.pointermove(pointer(1, 20, x));
    assert.equal(f.jobs.length, 0);
    assert.equal(root(f), base);
    f.task();
    assert.equal(f.jobs.length, 1);
    f.measure();
    assert.equal(f.jobs.length, 0);
    f.owner.pointermove(pointer(1, 21, 54));
    assert.equal(root(f), base);
    f.task();
    assert.equal(f.jobs.length, 1);
    f.measure();
    assert.equal(f.jobs.length, 0);
    f.owner.pointermove(pointer(1, 20, 55));
    assert.equal(root(f).childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1], tree);
    f.task();
    assert.equal(f.state.field(reorderPreview).value?.output.text, 'a, b, c');
    f.measure();
    f.task();
    drain(f);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [1, 2, 0]);
    assert.equal(root(f), base);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const release of ['pointerup', 'Alt'] as const)
  test(`cold calibration coalesces final ${release} without losing latest destination or one history event`, () => {
    const f = fixture();
    try {
      mount(f);
      const base = root(f);
      f.owner.pointermove(pointer(1, 20, 55));
      if (release === 'pointerup') f.owner.pointerup(pointer(1, 20, 55));
      else f.owner.keyup(key('Alt'));
      assert.equal(root(f), base);
      assert.equal(f.jobs.length, 0);
      drain(f);
      assert.equal(readExactSource(f.state), 'b, c, a');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(base.isConnected, false);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

for (const retirement of ['Escape', 'source'] as const)
  test(`old mounted calibration follow-up cannot schedule after ${retirement}`, () => {
    const f = fixture();
    try {
      mount(f);
      const base = root(f),
        old = f.tasks.shift();
      assert.ok(old);
      if (retirement === 'Escape') f.owner.keydown(key('Escape'));
      else f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      old();
      drain(f);
      assert.equal(base.isConnected, false);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(readExactSource(f.state), retirement === 'source' ? 'xa, b, c' : 'a, b, c');
      assert.equal(undoDepth(f.state), retirement === 'source' ? 1 : 0);
      assert.equal(f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

test('same-key geometry refresh retires the prior calibration issue and remeasures actual output', () => {
  const f = fixture();
  try {
    mount(f);
    const before = root(f),
      old = f.tasks.shift();
    assert.ok(old);
    f.owner.refreshGeometry();
    const pending = f.jobs.length;
    old();
    assert.equal(f.jobs.length, pending);
    drain(f);
    const after = root(f);
    assert.notEqual(after, before);
    assert.equal(before.isConnected, false);
    assert.equal(after.isConnected, true);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('a retired same-session hold cannot schedule its old calibration over the new held chip', () => {
  const c = pointerTransferFixture(),
    f = c.f;
  try {
    mount(f);
    const before = root(f),
      old = f.tasks.shift();
    assert.ok(old);
    f.owner.pointerdown(c.down(1, 9, 20, 45));
    drain(f);
    const preview = f.state.field(reorderPreview).value;
    assert.ok(preview);
    assert.equal(preview.activeChip, 1);
    old();
    drain(f);
    assert.equal(f.state.field(reorderPreview).value, preview);
    assert.equal(before.isConnected, false);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    c.destroy();
  }
});
/** A native context read failure at the deferred frontier preserves its primary cause and retires actual owned resources. */
test('throwing current native context retires the mounted calibration and reports the original failure', () => {
  const f = fixture();
  try {
    mount(f);
    const base = root(f),
      node = f.view.domAtPos(0).node as HTMLElement;
    const original = node.getAttribute.bind(node),
      failure = new Error('controlled-context-read');
    let armed = true;
    node.getAttribute = (name) => {
      if (armed && name === 'style') {
        armed = false;
        throw failure;
      }
      return original(name);
    };
    assert.throws(
      () => f.task(),
      (cause) => cause === failure,
    );
    drain(f);
    assert.equal(base.isConnected, false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});

/** A released gesture cannot depend on another pointer event to discover unavailable native context. */
test('released calibration with stale native context cancels finitely without further input', () => {
  const f = fixture();
  try {
    mount(f);
    const base = root(f),
      node = f.view.domAtPos(0).node as HTMLElement;
    f.owner.pointerup(pointer(1, 20, 55));
    node.className += ' controlled-context-change';
    drain(f);
    assert.equal(base.isConnected, false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});

/** Host appearance changes invalidate geometry while the unchanged held gesture still owns its source and native capture. */
test('held calibration renews changed native context through a fresh measured destination', () => {
  const f = fixture();
  try {
    mount(f);
    const before = root(f),
      node = f.view.domAtPos(0).node as HTMLElement,
      selection = f.state.selection;
    let previous = before;
    for (const marker of ['first', 'second']) {
      node.className += ` controlled-context-${marker}`;
      f.task();
      f.task();
      assert.equal(f.jobs.length, 1);
      f.measure();
      const renewed = root(f);
      assert.notEqual(renewed, previous);
      assert.equal(previous.isConnected, false);
      previous = renewed;
    }
    drain(f);
    const after = root(f);
    assert.notEqual(after, before);
    assert.equal(before.isConnected, false);
    assert.equal(after.isConnected, true);
    assert.ok(f.state.field(reorderPreview).value);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.captures.size, 1);
    f.owner.pointerup(pointer(1, 20, 55));
    drain(f);
    assert.equal(readExactSource(f.state), 'b, c, a');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(after.isConnected, false);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});

/** A real failed calibration read remains unavailable even when host appearance later changes before delivery. */
test('calibration geometry refusal remains terminal when its context later becomes stale', () => {
  const f = fixture();
  try {
    mount(f);
    const base = root(f),
      node = f.view.domAtPos(0).node as HTMLElement,
      box = base.getBoundingClientRect.bind(base);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    base.getBoundingClientRect = () => ({ ...box(), width: 0 });
    f.task();
    f.measure();
    node.className += ' controlled-later-context-change';
    drain(f);
    assert.equal(base.isConnected, false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});
