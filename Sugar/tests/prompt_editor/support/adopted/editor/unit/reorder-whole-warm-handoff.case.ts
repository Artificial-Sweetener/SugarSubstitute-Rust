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
/** Same-hold whole proposals reuse real Core/Widget/Stage/Presentation transport across admitted warm handoffs and reentry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redoDepth, undoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  reorderPreview,
  publishReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';
import { wholeFixture, publishWhole } from './reorder-pointer-stage-fixture.ts';

test('whole warm retarget keeps the actual gate and sheet without physical publication writes', () => {
  const f = wholeFixture();
  try {
    const selection = f.state.selection;
    f.prepare();
    f.deliver();
    const oldScene = f.paint.editor.querySelector('.sp-reorder-motion-paint'),
      old = f.state.field(reorderPreview).mask!,
      gate = f.view.dom.querySelector<HTMLElement>('[data-reorder-motion-gate]')!,
      sheet = f.paint.document
        .head!.querySelectorAll('style')
        .find((node) => node.textContent.includes('data-reorder-motion-gate'))!;
    assert.ok(old && oldScene && gate && sheet);
    const originalParent = oldScene.parentElement;
    assert.ok(originalParent);
    const originalGeometry = f.presentation.displayPaint(f.input)?.geometry;
    assert.ok(originalGeometry);
    const witnesses = oldScene.querySelectorAll('[data-reorder-chip]').map((node) => ({
      node,
      read: node.getClientRects,
      reads: 0,
    }));
    for (const witness of witnesses)
      witness.node.getClientRects = () => {
        ++witness.reads;
        return Reflect.apply(witness.read, witness.node, []) as DOMRect[];
      };
    const rules = sheet.textContent,
      set = gate.setAttribute.bind(gate),
      remove = gate.remove.bind(gate),
      sheetRemove = sheet.remove.bind(sheet),
      append = f.paint.document.head!.append.bind(f.paint.document.head!);
    /** Only owner metadata and the genuine new scene may change during warm gate handoff. */
    const forbidden = (): never => {
      throw new Error('whole-retarget-gate-write');
    };
    Object.defineProperty(sheet, 'textContent', {
      configurable: true,
      get: () => rules,
      set: forbidden,
    });
    gate.setAttribute = forbidden;
    gate.remove = forbidden;
    sheet.remove = forbidden;
    f.paint.document.head!.append = forbidden;
    try {
      publishWhole(f, 0);
      f.prepare();
      f.deliver();
      assert.equal(oldScene.parentElement === originalParent, true);
      assert.equal(oldScene.isConnected, true);
      assert.equal(oldScene.style.getPropertyPriority('clip'), 'important');
      assert.equal(oldScene.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.equal(oldScene.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      const next = f.state.field(reorderPreview).mask!;
      assert.ok(next);
      assert.notEqual(next.issue, old.issue);
      assert.equal(next.scope, old.scope);
      assert.equal(f.view.dom.querySelector('[data-reorder-motion-gate]'), gate);
      assert.equal(
        gate.getAttribute('data-reorder-motion-active'),
        reorderPaintIssueName(next.scope),
      );
      assert.equal(sheet.textContent, rules);
      assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
      assert.equal(f.frames.size, 1);
      f.step(200);
      assert.equal(f.frames.size, 0);
      assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
      assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
      publishWhole(f, 2);
      f.prepare();
      f.deliver();
      const returned = f.paint.editor
        .querySelectorAll('.sp-reorder-motion-paint')
        .find((node) => node.style.clip === 'auto');
      assert.equal(returned, oldScene);
      assert.equal(oldScene.parentElement, originalParent);
      assert.equal(oldScene.isConnected, true);
      assert.ok(witnesses.length > 0);
      assert.ok(witnesses.every((witness) => witness.reads === 1));
      const returnedGeometry = f.presentation.displayPaint(f.input)?.geometry;
      assert.notEqual(returnedGeometry, originalGeometry);
      assert.deepEqual(returnedGeometry, originalGeometry);
      assert.equal(returned!.style.visibility, 'visible');
      assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
      f.step(400);
      assert.equal(f.frames.size, 0);
      assert.equal(f.state.selection, selection);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      for (const witness of witnesses) witness.node.getClientRects = witness.read;
      gate.setAttribute = set;
      gate.remove = remove;
      sheet.remove = sheetRemove;
      f.paint.document.head!.append = append;
      Reflect.deleteProperty(sheet, 'textContent');
    }
  } finally {
    f.destroy();
  }
});

test('pending foreign native mutation refuses warm transfer before uncovering either scene', () => {
  let observer: ReturnType<typeof visibilityObserverFixture> | null = null;
  const f = wholeFixture((document) => {
    observer = visibilityObserverFixture(document);
  });
  assert.ok(observer);
  try {
    f.prepare();
    f.deliver();
    const oldScene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
      set = oldScene.style.setProperty.bind(oldScene.style);
    oldScene.style.setProperty = (name, value, priority) => {
      if (name === 'clip' && value === 'rect(0px, 0px, 0px, 0px)')
        assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
      set(name, value, priority);
    };
    publishWhole(f, 0);
    f.root.setAttribute('data-foreign', '1');
    f.prepare();
    f.deliver();
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(f.paint.editor.querySelectorAll('[data-reorder-motion-endpoint]').length, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

for (const raw of [null, undefined])
  test(`whole transfer getter failure preserves raw ${String(raw)} and restores the current proposal before uncover`, () => {
    const f = wholeFixture();
    try {
      f.prepare();
      f.deliver();
      const sheet = f.paint.document
          .head!.querySelectorAll('style')
          .find((node) => node.textContent.includes('data-reorder-motion-gate'))!,
        oldScene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
        set = oldScene.style.setProperty.bind(oldScene.style);
      oldScene.style.setProperty = (name, value, priority) => {
        if (name === 'clip' && value === 'rect(0px, 0px, 0px, 0px)')
          assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
        set(name, value, priority);
      };
      Object.defineProperty(sheet, 'textContent', {
        configurable: true,
        get() {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        },
      });
      publishWhole(f, 0);
      f.prepare();
      assert.throws(
        () => f.deliver(),
        (cause) => cause === raw,
      );
      f.microtasks();
      assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });

for (const raw of [null, undefined])
  test(`whole retarget clock cancellation retains raw ${String(raw)} and attempts both scene retirements`, () => {
    const f = wholeFixture();
    const window = f.view.dom.ownerDocument.defaultView!;
    const cancel = window.cancelAnimationFrame.bind(window);
    try {
      f.prepare();
      f.deliver();
      window.cancelAnimationFrame = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      };
      publishWhole(f, 0);
      f.prepare();
      assert.throws(
        () => f.deliver(),
        (cause) => cause === raw,
      );
      window.cancelAnimationFrame = cancel;
      f.microtasks();
      f.step(0);
      assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
      assert.equal(f.paint.editor.querySelectorAll('[data-reorder-motion-endpoint]').length, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      window.cancelAnimationFrame = cancel;
      f.destroy();
    }
  });

for (const change of ['hold-anchor', 'context'] as const)
  test(`whole retarget cannot reuse the old physical gate after ${change} replacement`, () => {
    const f = wholeFixture();
    try {
      f.prepare();
      f.deliver();
      const oldMask = f.state.field(reorderPreview).mask!,
        gate = f.view.dom.querySelector<HTMLElement>('[data-reorder-motion-gate]')!,
        selection = f.state.selection,
        transitions: string[] = [],
        set = gate.setAttribute.bind(gate);
      gate.setAttribute = (name, value) => {
        set(name, value);
        if (name === 'data-reorder-motion-active') transitions.push(gate.getAttribute(name) ?? '');
      };
      if (change === 'hold-anchor') {
        const preview = f.state.field(reorderPreview).value!,
          hold = createReorderApplicationHold(preview.order),
          order = moveReorderChip(preview.capture.prepared, preview.order, 0, {
            kind: 'row',
            row: 0,
            insertion: 0,
          });
        assert.ok(order);
        assert.equal(
          publishReorderPreview(f.view, { ...preview, order }, 'pointer', hold, () => true),
          'published',
        );
        const state = f.state.field(reorderPreview);
        assert.equal(new ReorderWidget(state.paint!, null).updateDOM(f.root, f.view), true);
        Object.assign(f.input, { order });
      } else {
        f.view.dom.setAttribute('data-appearance-replaced', '1');
        publishWhole(f, 0);
      }
      f.prepare();
      f.deliver();
      f.microtasks();
      assert.equal(transitions.includes(''), true);
      const mask = f.state.field(reorderPreview).mask;
      if (mask) {
        assert.notEqual(mask.issue, oldMask.issue);
        const active = f.view.dom.querySelector<HTMLElement>('[data-reorder-motion-gate]');
        assert.equal(
          active?.getAttribute('data-reorder-motion-active'),
          reorderPaintIssueName(mask.scope),
        );
      } else assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
      assert.equal(f.state.selection, selection);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });

test('foreign mutation inside the whole visibility receipt cannot escape into JS gate transfer', () => {
  const f = wholeFixture((document) => {
    visibilityObserverFixture(document);
  });
  try {
    f.prepare();
    f.deliver();
    const old = f.state.field(reorderPreview).mask!,
      get = f.view.contentDOM.getAttribute.bind(f.view.contentDOM);
    let changed = false;
    f.view.contentDOM.getAttribute = (name) => {
      const value = get(name),
        mask = f.state.field(reorderPreview).mask;
      if (!changed && name === 'style' && mask && mask.issue !== old.issue) {
        changed = true;
        f.root.setAttribute('data-foreign-receipt', '1');
      }
      return value;
    };
    publishWhole(f, 0);
    f.prepare();
    f.deliver();
    f.microtasks();
    assert.equal(changed, true);
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.paint.editor.querySelectorAll('[data-reorder-motion-endpoint]').length, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

for (const raw of [null, undefined])
  test(`old whole Scene cleanup failure ${String(raw)} retires the candidate before reveal`, () => {
    const f = wholeFixture();
    try {
      f.prepare();
      f.deliver();
      const oldScene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
        set = oldScene.style.setProperty.bind(oldScene.style);
      let failed = false;
      oldScene.style.setProperty = (name, value, priority) => {
        set(name, value, priority);
        if (!failed && name === 'clip' && value === 'rect(0px, 0px, 0px, 0px)') {
          failed = true;
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw raw;
        }
      };
      publishWhole(f, 0);
      f.prepare();
      assert.throws(
        () => f.deliver(),
        (cause) => cause === raw,
      );
      f.microtasks();
      assert.equal(failed, true);
      assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.paint.editor.querySelectorAll('[data-reorder-motion-endpoint]').length, 0);
      assert.equal(f.frames.size, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });

for (const frontier of ['transfer-getter', 'old-cleanup'] as const)
  test(`same-hold proposal reentry at ${frontier} retains frame-consistent accepted paint until the next native READ`, () => {
    const f = wholeFixture();
    try {
      f.prepare();
      f.deliver();
      f.step(80);
      const oldMask = f.state.field(reorderPreview).mask!,
        oldScene = f.paint.editor.querySelector('.sp-reorder-motion-paint')!,
        sheet = f.paint.document
          .head!.querySelectorAll('style')
          .find((node) => node.textContent.includes('data-reorder-motion-gate'))!,
        rules = sheet.textContent;
      let entered = false,
        newer = f.state.field(reorderPreview).value;
      /** Future native READ delivery stays outside this synchronous adoption callback. */
      const replace = (): void => {
        if (entered) return;
        entered = true;
        publishWhole(f, 1);
        newer = f.state.field(reorderPreview).value;
        f.prepare();
      };
      if (frontier === 'transfer-getter')
        Object.defineProperty(sheet, 'textContent', {
          configurable: true,
          get() {
            replace();
            return rules;
          },
        });
      else {
        const set = oldScene.style.setProperty.bind(oldScene.style);
        oldScene.style.setProperty = (name, value, priority) => {
          if (name === 'clip' && value === 'rect(0px, 0px, 0px, 0px)') replace();
          set(name, value, priority);
        };
      }
      publishWhole(f, 0);
      f.prepare();
      f.deliver();
      assert.equal(entered, true);
      assert.equal(f.state.field(reorderPreview).value, newer);
      assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
      const mask = f.state.field(reorderPreview).mask!;
      assert.ok(mask);
      if (frontier === 'transfer-getter') assert.equal(mask.issue, oldMask.issue);
      else assert.notEqual(mask.issue, oldMask.issue);
      const accepted = f.paint.editor
        .querySelectorAll('.sp-reorder-motion-paint')
        .find((node) => node.dataset.reorderMotionEndpoint === reorderPaintIssueName(mask.issue));
      assert.ok(accepted);
      assert.equal(accepted.style.clip, 'auto');
      const frame = f.presentation.before();
      for (const root of accepted.querySelectorAll('[data-reorder-motion-chip]')) {
        const id = Number(root.dataset.reorderMotionChip),
          point = frame.find((rect) => rect.kind === 'chip' && rect.index === id),
          box = root.getBoundingClientRect();
        assert.ok(point);
        assert.equal(
          point.left,
          box.left + Number.parseFloat(root.style.getPropertyValue('left') || '0'),
        );
        assert.equal(
          point.top,
          box.top + Number.parseFloat(root.style.getPropertyValue('top') || '0'),
        );
      }
      assert.ok(f.jobs.length > 0);
      f.deliver();
      assert.equal(f.state.field(reorderPreview).value, newer);
      assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });
