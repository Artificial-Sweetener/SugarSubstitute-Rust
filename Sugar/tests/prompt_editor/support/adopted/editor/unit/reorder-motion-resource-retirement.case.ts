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
/** Context retirement owns scene output, terminal physical removal and the captured stylesheet independently. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('disposing passive presentation releases its exact mask on the retained native view', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.deliver();
    const mask = f.state.field(reorderPreview).mask,
      selection = f.state.selection,
      scene = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(mask);
    assert.ok(scene);
    f.presentation.destroy();
    assert.equal(scene.isConnected, false);
    assert.equal(f.frames.size, 0);
    assert.equal(f.document.head!.childNodes.length, 0);
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('scene and stylesheet retirement attempts both resources and preserves ordered original cleanup causes', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.deliver();
    const scene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
      style = f.document.head!.childNodes[1] as (typeof f.chips)[number];
    const first = new Error('scene-cleanup'),
      second = new Error('style-cleanup'),
      attempted: string[] = [];
    const sceneRemove = scene.remove.bind(scene),
      styleRemove = style.remove.bind(style);
    const set = scene.style.setProperty.bind(scene.style);
    scene.style.setProperty = (name, value, priority) => {
      if (name === 'clip') {
        attempted.push('scene');
        throw first;
      }
      set(name, value, priority);
    };
    scene.remove = () => {
      attempted.push('scene');
      sceneRemove();
      throw first;
    };
    style.remove = () => {
      attempted.push('style');
      styleRemove();
      throw second;
    };
    assert.throws(
      () => f.presentation.cancel(),
      (cause: unknown) => {
        assert.ok(cause instanceof AggregateError);
        assert.equal(cause.message, 'reorder-motion-context-retirement-failed');
        assert.equal(cause.errors.length, 2);
        const slotFailure: unknown = cause.errors[0];
        assert.ok(slotFailure instanceof AggregateError);
        assert.equal(slotFailure.message, 'Passive motion output retirement failed');
        // Failed whole-idle clipping and independent terminal removal preserve both exact raw causes.
        assert.equal(slotFailure.errors.length, 2);
        assert.equal(slotFailure.errors[0], first);
        assert.equal(slotFailure.errors[1], first);
        assert.equal(cause.errors[1], second);
        return true;
      },
    );
    assert.deepEqual(attempted, ['scene', 'scene', 'style']);
    assert.equal(f.frames.size, 0);
    assert.equal(f.document.head!.childNodes.length, 0);
    assert.equal(scene.isConnected, false);
    while (f.tasks.length) f.tasks.shift()!();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('cancellation snapshots the old sheet before scene-clip reentry admits a newer actual issue', () => {
  const f = motionPresentationFixture();
  /** Literal native replacement transport permits the real widget to publish a newer mounted lease. */
  f.root.replaceChildren = (...nodes: (Node | string)[]) => {
    for (const child of Array.from(f.root.childNodes)) (child as HTMLElement).remove();
    f.root.append(...nodes);
  };
  try {
    f.prepare();
    f.deliver();
    const oldIssue = f.state.field(reorderPreview).mask!.issue;
    const scene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
      oldStyle = f.document.head!.childNodes[1]!;
    assert.ok('isConnected' in oldStyle);
    let entered = false;
    const set = scene.style.setProperty.bind(scene.style);
    scene.style.setProperty = (name, value, priority) => {
      set(name, value, priority);
      if (name !== 'clip') return;
      const gate = f.view.dom.querySelector('[data-reorder-motion-gate]');
      assert.ok(gate);
      assert.equal(gate.getAttribute('data-reorder-motion-active') || '', '');
      if (!entered) {
        entered = true;
        f.publish();
        const publication = f.state.field(reorderPreview).value!;
        assert.equal(
          new ReorderWidget(publication, f.state.field(reorderPreview).mask).updateDOM(
            f.root,
            f.view,
          ),
          true,
        );
        f.prepare();
        f.deliver();
      }
    };
    f.presentation.cancel();
    assert.equal(entered, true);
    const newer = f.state.field(reorderPreview).mask;
    assert.ok(newer);
    assert.notEqual(newer.issue, oldIssue);
    assert.equal(oldStyle.isConnected, false);
    assert.equal(f.document.head!.childNodes.length, 2);
    const gate = f.view.dom.querySelector('[data-reorder-motion-gate]');
    assert.ok(gate);
    assert.equal(
      gate.getAttribute('data-reorder-motion-active'),
      reorderPaintIssueName(newer.scope),
    );
    while (f.tasks.length) f.tasks.shift()!();
    assert.equal(f.state.field(reorderPreview).mask, newer);
    assert.equal(f.frames.size, 1);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
