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
/** Stage-owned canonical slots bound simultaneous construction, idle reuse and raw teardown reentry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('accepted plus pending owns two actual shells and obsolete callbacks cannot retire the next lease', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const accepted = stage.mount(prepared),
      pending = stage.mount(prepared);
    assert.ok(accepted && pending);
    const old = accepted.node,
      other = pending.node;
    assert.equal(stage.mount(prepared), null);
    assert.equal(stage.node.querySelectorAll('.sp-reorder-motion-paint').length, 2);
    pending.destroy();
    assert.equal(other.isConnected, true);
    assert.ok(other.parentElement === stage.node);
    assert.equal(other.dataset.reorderMotionProgress, undefined);
    assert.equal(other.style.clip, 'rect(0px, 0px, 0px, 0px)');
    const replacement = stage.mount(prepared);
    assert.ok(replacement);
    assert.ok(replacement.node === other);
    pending.destroy();
    assert.equal(replacement.current(), true);
    assert.equal(accepted.current(), true);
    assert.ok(accepted.node === old);
    assert.equal(stage.node.querySelectorAll('.sp-reorder-motion-paint').length, 2);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});

test('in-flight first construction occupies a reservation before reentrant second and refused third mounts', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const owner = stage,
      create = f.paint.document.createElement.bind(f.paint.document);
    let depth = 0,
      second: ReturnType<typeof owner.mount> | undefined,
      third: ReturnType<typeof owner.mount> | undefined;
    f.paint.document.createElement = (name) => {
      const node = create(name);
      if (depth < 2) {
        depth++;
        if (depth === 1) second = owner.mount(prepared);
        else third = owner.mount(prepared);
      }
      return node;
    };
    const first = stage.mount(prepared);
    assert.ok(first && second);
    assert.equal(third, null);
    assert.equal(stage.node.querySelectorAll('.sp-reorder-motion-paint').length, 2);
    assert.equal(first.current(), true);
    assert.equal(second.current(), true);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});

for (const fails of [false, true])
  test(`retiring idle slot stays counted through native cleanup reentry${fails ? ' and raw failure' : ''}`, () => {
    const f = motionPresentationFixture();
    let stage: ReorderPassiveStage | null = null;
    try {
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const accepted = stage.mount(prepared),
        pending = stage.mount(prepared);
      assert.ok(accepted && pending);
      const shell = pending.node;
      pending.destroy();
      const remove = shell.remove.bind(shell),
        cause = new Error('controlled-idle-retirement'),
        owner = stage;
      let attempts = 0;
      shell.remove = () => {
        attempts++;
        assert.equal(owner.mount(prepared), null);
        remove();
        if (fails) throw cause;
      };
      const destroy = ReorderWidget.prototype.destroy;
      let paintReleases = 0;
      /** Observe the actual cold-discard painter after its external shell detaches. */
      ReorderWidget.prototype.destroy = function (dom) {
        if (shell.contains(dom)) {
          paintReleases++;
          assert.equal(shell.isConnected, false);
        }
        destroy.call(this, dom);
      };
      const changed = Object.freeze({
        ...prepared,
        contentCss: prepared.contentCss + 'color: blue;',
      });
      try {
        if (fails) {
          assert.throws(
            () => owner.mount(changed),
            (error) => error === cause,
          );
          assert.equal(owner.mount(prepared), null);
        } else assert.ok(owner.mount(changed));
        assert.equal(paintReleases, 1);
      } finally {
        ReorderWidget.prototype.destroy = destroy;
      }
      assert.equal(attempts, 1);
      assert.equal(accepted.current(), true);
      assert.equal(accepted.node.isConnected, true);
    } finally {
      try {
        stage?.destroy();
      } finally {
        f.destroy();
      }
    }
  });

test('warm update raw failure preserves accepted output and primary then cleanup causes', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const accepted = stage.mount(prepared),
      pending = stage.mount(prepared);
    assert.ok(accepted && pending);
    const shell = pending.node,
      widget = shell.querySelector<HTMLElement>('.sp-reorder-preview');
    assert.ok(widget);
    pending.destroy();
    const primary = new Error('controlled-update'),
      cleanup = new Error('controlled-cleanup'),
      remove = shell.remove.bind(shell);
    const target = widget.querySelector<HTMLElement>('[data-reorder-chip]');
    assert.ok(target);
    target.contains = () => {
      throw primary;
    };
    shell.remove = () => {
      remove();
      throw cleanup;
    };
    assert.throws(
      () => stage!.mount(prepared),
      (error) =>
        error instanceof AggregateError &&
        error.errors[0] === primary &&
        error.errors[1] === cleanup,
    );
    assert.equal(accepted.current(), true);
    assert.equal(accepted.node.isConnected, true);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});

test('warm wrapper output reset retirement stops before retired-line borrowing and returns null', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const first = stage.mount(prepared);
    assert.ok(first);
    first.destroy();
    const owner = stage;
    let retired = false;
    Object.defineProperty(first.node.style, 'left', {
      configurable: true,
      set() {
        retired = true;
        owner.destroy();
      },
    });
    assert.equal(owner.mount(prepared) === null, true);
    assert.equal(retired, true);
    assert.equal(owner.mount(prepared) === null, true);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});

test('canceling only the in-flight warm child cannot cold-retry the old continuation under a still-current Stage', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    stage = new ReorderPassiveStage(f.view, prepared);
    const accepted = stage.mount(prepared),
      idle = stage.mount(prepared);
    assert.ok(accepted && idle);
    idle.destroy();
    const owner = stage;
    let canceled = false,
      resets = 0;

    // The retained shell's native reset is the real warm write frontier; canonical children are not allocated again.
    const widget = idle.node.querySelector<HTMLElement>('.sp-reorder-preview');
    assert.ok(widget);
    Object.defineProperty(idle.node.style, 'left', {
      configurable: true,
      set() {
        resets++;
        if (!canceled) {
          canceled = true;
          new ReorderWidget(prepared.preview, null).destroy(widget);
        }
      },
    });
    assert.equal(owner.mount(prepared) === null, true);
    assert.equal(canceled, true);
    assert.equal(resets, 1);
    assert.equal(owner.current(), true);
    assert.equal(accepted.current(), true);
  } finally {
    try {
      stage?.destroy();
    } finally {
      f.destroy();
    }
  }
});

for (const removal of ['success', 'before', 'after'] as const)
  test(`terminal shell retirement attempts all resource cleanup after ${removal} removal`, () => {
    const fails = removal !== 'success';
    const f = motionPresentationFixture(),
      state = f.state,
      mount = ReorderWidget.prototype.mountPassive;
    let stage: ReorderPassiveStage | null = null;
    const calls: number[] = [],
      detachFailure = Object.freeze({ code: 'controlled-detach' });
    try {
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const owner = stage;
      let shell: HTMLElement | null = null;
      /** Delegate actual media and canonical cleanup; only the external native boundary injects failures. */
      ReorderWidget.prototype.mountPassive = function (view, media) {
        const paint = mount.call(this, view, media);
        assert.ok(media);
        const destroyMedia = media.destroy.bind(media);
        media.destroy = () => {
          calls.push(1);
          assert.equal(shell?.isConnected, removal === 'before');
          destroyMedia();
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          if (fails) throw null;
        };
        return Object.freeze({
          ...paint,
          destroy() {
            calls.push(2);
            assert.equal(shell?.isConnected, removal === 'before');
            paint.destroy();
            assert.equal(paint.targets().length, 0);
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            if (fails) throw undefined;
          },
        });
      };
      const pending = owner.mount(prepared);
      assert.ok(pending);
      shell = pending.node;
      assert.equal(shell.isConnected, true);
      const remove = shell.remove.bind(shell);
      shell.remove = () => {
        calls.push(0);
        assert.equal(pending.current(), false);
        assert.equal(owner.mount(prepared) === null, true);
        owner.destroy();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (removal === 'before') throw detachFailure;
        remove();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (fails) throw detachFailure;
      };
      if (fails)
        assert.throws(
          () => owner.destroy(),
          (error) =>
            error instanceof AggregateError &&
            error.errors.length === 3 &&
            error.errors[0] === detachFailure &&
            error.errors[1] === null &&
            error.errors[2] === undefined,
        );
      else owner.destroy();
      assert.deepEqual(calls, [0, 1, 2]);
      assert.equal(shell.isConnected, false);
      assert.equal(f.state.doc, state.doc);
      assert.equal(f.state.selection, state.selection);
      assert.equal(readExactSource(f.state), readExactSource(state));
      assert.equal(undoDepth(f.state), undoDepth(state));
      assert.equal(redoDepth(f.state), redoDepth(state));
      assert.equal(f.state.field(reorderPreview).value, state.field(reorderPreview).value);
      assert.equal(f.state.field(reorderPreview).mask, state.field(reorderPreview).mask);
      owner.destroy();
      pending.destroy();
      assert.deepEqual(calls, [0, 1, 2]);
    } finally {
      ReorderWidget.prototype.mountPassive = mount;
      try {
        stage?.destroy();
      } finally {
        f.destroy();
      }
    }
  });
