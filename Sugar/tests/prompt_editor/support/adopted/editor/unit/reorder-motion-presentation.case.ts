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
/** The real presentation uses genuine CM capture/publication/mounted paint, with native DOM/style/timing as controlled boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';

test('pending native target reads retain the actual issued landing and changed or stale acceptance retires it', () => {
  const f = motionPresentationFixture(),
    viewport = { left: 0, right: 200, top: 0, bottom: 100 };
  try {
    f.presentation.mark(f.input, f.origin, 'rgb(1, 2, 3)', viewport, false);
    const surface = f.paint.document.body!.childNodes.at(-1)!;
    assert.ok('style' in surface);
    const svg = surface.childNodes[0]!,
      path = svg.childNodes[0]!;
    const allocations = f.paint.allocations;
    f.presentation.pending();
    assert.equal(surface.style.opacity, '0.52');
    f.presentation.mark(f.input, f.origin, 'rgb(1, 2, 3)', viewport, false);
    assert.equal(f.paint.document.body!.childNodes.at(-1), surface);
    assert.equal(svg.childNodes[0], path);
    assert.equal(surface.style.opacity, '0.82');
    assert.equal(f.paint.allocations, allocations);
    f.presentation.mark(
      f.input,
      f.origin.map((rect) => ({
        ...rect,
        left: rect.left + 10,
        right: rect.right + 10,
      })),
      'rgb(1, 2, 3)',
      viewport,
      false,
    );
    assert.equal(surface.isConnected, false);
    assert.notEqual(f.paint.document.body!.childNodes.at(-1), surface);
    f.dispatch({ selection: EditorSelection.range(4, 1) });
    f.presentation.pending();
    assert.equal(f.paint.document.body!.childNodes.length, 1);
    f.presentation.cancel();
    assert.equal(f.frames.size, 0);
  } finally {
    f.destroy();
  }
});
import { EditorSelection, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { ReorderMotionPresentation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-presentation.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

test('landing cancellation preserves its removal cause and cannot revive retired pending paint', () => {
  const f = motionPresentationFixture(),
    cause = new Error('landing-removal-failed');
  try {
    f.presentation.mark(
      f.input,
      f.origin,
      'rgb(1, 2, 3)',
      { left: 0, right: 200, top: 0, bottom: 100 },
      false,
    );
    const surface = f.paint.document.body!.childNodes.at(-1)!;
    assert.ok('remove' in surface);
    const remove = surface.remove;
    surface.remove = () => {
      Reflect.apply(remove, surface, []);
      throw cause;
    };
    assert.throws(
      () => f.presentation.cancel(),
      (observed) => observed === cause,
    );
    f.presentation.pending();
    assert.equal(surface.isConnected, false);
    assert.equal(f.paint.document.body!.childNodes.length, 1);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).mask, null);
  } finally {
    f.destroy();
  }
});
test('complete actual paint adopts one exact mask and reuses its populated scene without new native reads or allocations', () => {
  const f = motionPresentationFixture();
  try {
    const selection = f.state.selection;
    f.prepare();
    f.deliver();
    const mask = f.state.field(reorderPreview).mask;
    assert.ok(mask);
    const scene = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(scene);
    const reads = f.paint.styleReads,
      allocations = f.paint.allocations;
    assert.equal(f.presentation.hit(65, 25), 1);
    f.paint.step(90);
    assert.equal(f.presentation.hit(35, 25), 1);
    assert.equal(f.paint.styleReads, reads);
    assert.equal(f.paint.allocations, allocations);
    f.prepare();
    assert.equal(f.jobs.length, 0);
    assert.equal(f.state.field(reorderPreview).mask, mask);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    const retainedStyle = f.paint.document.head!.childNodes[1]!;
    f.paint.step(180);
    assert.equal(f.paint.document.head!.childNodes.length, 2);
    assert.equal(f.paint.document.head!.childNodes[1], retainedStyle);
    const gate = f.view.dom.querySelector('[data-reorder-motion-gate]');
    assert.ok(gate);
    assert.equal(gate.getAttribute('data-reorder-motion-active') || '', '');
    assert.equal(scene.dataset.reorderMotionProgress, undefined);
    assert.equal(scene.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.ok(scene.parentElement === f.paint.editor.querySelector('.sp-reorder-motion-stage'));
    assert.ok(scene.parentElement?.parentElement === f.paint.layerRoot.childNodes[0]);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 1);
    assert.equal(scene.isConnected, true);
    assert.equal(scene.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.equal(f.presentation.hit(25, 25), null);
  } finally {
    f.destroy();
  }
});
test('canonical selection retirement rejects stale visible identity and press paint before the native plugin can retire resources', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.deliver();
    assert.equal(f.presentation.hit(65, 25), 1);
    f.dispatch({ selection: EditorSelection.range(4, 1) });
    assert.equal(f.presentation.hit(65, 25), null);
    assert.equal(f.presentation.reserveGrab(1, { x: 65, y: 25 }), false);
    assert.equal(f.state.field(reorderPreview).mask, null);
    f.presentation.cancel();
    assert.equal(f.frames.size, 0);
    assert.equal(f.paint.document.head!.childNodes.length, 0);
    assert.equal(f.paint.document.body!.childNodes.length, 1);
  } finally {
    f.destroy();
  }
});
test('actual painted press transfers a bounded complete lease with the original visual hotspot', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    f.deliver();
    f.paint.step(90);
    assert.equal(f.presentation.reserveGrab(1, { x: 35, y: 25 }), true);
    assert.equal(f.frames.size, 0);
    const grabbed = f.presentation.captureGrab(1);
    assert.ok(grabbed);
    assert.equal(grabbed.kind, 'paint');
    if (grabbed.kind !== 'paint') assert.fail('painted-grab-unavailable');
    assert.deepEqual(
      [
        grabbed.paint.geometry.left,
        grabbed.paint.geometry.top,
        grabbed.paint.geometry.grabX,
        grabbed.paint.geometry.grabY,
      ],
      [30, 20, 5, 5],
    );
    assert.equal(f.presentation.captureGrab(1), null);
    grabbed.paint.node.remove();
  } finally {
    f.destroy();
  }
});
test('same-source preparation reentry during owned stylesheet adoption cannot publish an older display issue', () => {
  const f = motionPresentationFixture();
  try {
    const head = f.paint.document.head!,
      append = head.append.bind(head);
    let entered = false;
    head.append = (...nodes) => {
      append(...nodes);
      if (
        !entered &&
        nodes.some((node) => node.textContent.includes('data-reorder-motion-local'))
      ) {
        entered = true;
        f.prepare();
      }
    };
    f.prepare();
    f.deliver();
    assert.equal(entered, true);
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(head.childNodes.length, 2);
    const gate = f.view.dom.querySelector('[data-reorder-motion-gate]');
    // This adoption retired before attaching its marker; absent and inactive markers both leave native paint unsuppressed.
    assert.equal(gate?.getAttribute('data-reorder-motion-active') ?? '', '');
    assert.equal(f.jobs.length, 1);
    const idle = f.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
    assert.ok(idle);
    assert.equal(idle.dataset.reorderMotionProgress, undefined);
    assert.equal(idle.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.ok(idle.parentElement === f.view.dom.querySelector('.sp-reorder-motion-stage'));
    assert.ok(idle.parentElement?.parentElement === f.paint.layerMount);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 1);
    assert.equal(idle.isConnected, true);
    assert.equal(idle.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('partial motion-policy construction retires its registered listener and retains exact original failure', () => {
  const f = motionPaintFixture(),
    cause = new Error('motion-policy-registration'),
    removed: string[] = [];
  Object.assign(f.document.defaultView, {
    matchMedia() {
      return {
        matches: false,
        addEventListener() {
          throw cause;
        },
        removeEventListener(name: string) {
          removed.push(name);
        },
      };
    },
  });
  assert.throws(
    () =>
      new ReorderMotionPresentation(f.view, {
        current: () => false,
        rects: () => [],
        refresh() {},
        observe: (action) => action(),
        schedule() {},
      }),
    (error) => error === cause,
  );
  assert.deepEqual(removed, ['change']);
  assert.equal(f.frames.size, 0);
  assert.equal(f.document.head!.childNodes.length, 0);
  assert.equal(f.document.body!.childNodes.length, 1);
});

/** Actual scene completion cannot certify a newer unresolved pointer intent as a fresh landing. */
test('scene settlement retains pending landing dim until accepted geometry is marked fresh', () => {
  const f = motionPresentationFixture(),
    viewport = { left: 0, right: 200, top: 0, bottom: 100 };
  try {
    f.prepare();
    f.presentation.mark(f.input, f.origin, 'rgb(1, 2, 3)', viewport, true);
    const surface = f.paint.document.body!.childNodes.at(-1)!;
    assert.ok('style' in surface);
    assert.equal(surface.style.opacity, '0.52');
    f.deliver();
    assert.equal(surface.style.opacity, '0.52');
    assert.equal(surface.isConnected, true);
    f.presentation.mark(f.input, f.origin, 'rgb(1, 2, 3)', viewport, false);
    assert.equal(surface.style.opacity, '0.82');
    assert.equal(surface.isConnected, true);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

/** Stage creation and accepted Resource adoption are distinct external reentry frontiers. */
test('stage stylesheet reentry retires the older constructor before explicitly accepting the newer request', () => {
  const f = motionPresentationFixture();
  try {
    const head = f.paint.document.head!,
      append = head.append.bind(head);
    let entered = false;
    head.append = (...nodes) => {
      append(...nodes);
      if (
        !entered &&
        nodes.some((node) => node.textContent.includes('data-reorder-motion-stage'))
      ) {
        entered = true;
        f.publish();
        f.prepare();
      }
    };
    f.prepare();
    f.jobs.shift()!();
    f.tasks.shift()!();
    assert.equal(entered, true);
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(head.childNodes.length, 0);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
    assert.equal(f.jobs.length, 1);
    const supersedingCaptureAndRetiredLayer = f.jobs.shift();
    assert.ok(supersedingCaptureAndRetiredLayer);
    supersedingCaptureAndRetiredLayer();
    assert.equal(f.lastRead, 'native-layer');
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(head.childNodes.length, 0);
    f.microtasks();
    assert.equal(head.childNodes.length, 1);
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(f.jobs.length, 1);
    f.deliver();
    assert.ok(f.state.field(reorderPreview).mask);
    assert.equal(f.jobs.length, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('Resource stylesheet allocation reentry cannot overwrite a newer pending request', () => {
  const f = motionPresentationFixture();
  try {
    const create = f.paint.document.createElement.bind(f.paint.document);
    let styles = 0;
    f.paint.document.createElement = (name) => {
      const node = create(name);
      if (name === 'style' && ++styles === 2) f.prepare();
      return node;
    };
    f.prepare();
    f.deliver();
    assert.equal(styles, 2);
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.jobs.length, 1);
    assert.equal(f.frames.size, 0);
    assert.equal(f.paint.document.head!.childNodes.length, 1);
    const idle = f.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
    assert.ok(idle);
    assert.equal(idle.dataset.reorderMotionProgress, undefined);
    assert.equal(idle.style.clip, 'rect(0px, 0px, 0px, 0px)');
    assert.ok(idle.parentElement === f.view.dom.querySelector('.sp-reorder-motion-stage'));
    assert.ok(idle.parentElement?.parentElement === f.paint.layerMount);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 1);
    assert.equal(idle.isConnected, true);
    assert.equal(idle.querySelectorAll('[data-reorder-motion-chip]').length, 0);
  } finally {
    f.destroy();
  }
});
for (const primary of [null, undefined, new Error('controlled-resource-style-allocation')] as const)
  test(`Resource style allocation retains raw primary ${String(primary)} and retires actual canonical output`, () => {
    const f = motionPresentationFixture();
    try {
      const create = f.paint.document.createElement.bind(f.paint.document);
      let styles = 0;
      f.paint.document.createElement = (name) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'style' && ++styles === 2) throw primary;
        return create(name);
      };
      f.prepare();
      let failed = false,
        actual: unknown;
      try {
        f.deliver();
      } catch (cause) {
        failed = true;
        actual = cause;
      }
      assert.equal(failed, true);
      assert.equal(actual, primary);
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
      const idle = f.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-paint');
      assert.ok(idle);
      assert.equal(idle.dataset.reorderMotionProgress, undefined);
      assert.equal(idle.style.clip, 'rect(0px, 0px, 0px, 0px)');
      assert.ok(idle.parentElement === f.view.dom.querySelector('.sp-reorder-motion-stage'));
      assert.ok(idle.parentElement?.parentElement === f.paint.layerMount);
      assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-stage').length, 1);
      assert.equal(idle.isConnected, true);
      assert.equal(idle.querySelectorAll('[data-reorder-motion-chip]').length, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('actual mask facet scope publication preserves the calibrated canonical appearance lease', () => {
  const f = motionPresentationFixture();
  const dispatch = f.view.dispatch.bind(f.view);
  /** Initial native mount and each later dispatch consume the same actual supported editor-attribute facet. */
  const attributes = (): void => {
    for (const values of f.state.facet(EditorView.editorAttributes)) {
      if (typeof values === 'function') continue;
      for (const [name, value] of Object.entries(values))
        if (typeof value === 'string') f.view.dom.setAttribute(name, value);
    }
  };
  f.view.dispatch = ((spec: TransactionSpec) => {
    dispatch(spec);
    attributes();
  }) as typeof f.view.dispatch;
  try {
    attributes();
    f.prepare();
    f.deliver();
    const mask = f.state.field(reorderPreview).mask;
    assert.ok(mask);
    assert.equal(
      f.view.dom.getAttribute('data-reorder-motion-scope'),
      reorderPaintIssueName(mask.scope),
    );
    f.paint.step(90);
    assert.equal(f.state.field(reorderPreview).mask, mask);
    assert.equal(f.frames.size, 1);
    assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 1);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
