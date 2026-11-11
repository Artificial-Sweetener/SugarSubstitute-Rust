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
/** Whole pointer presentation composes actual Core, Widget, Stage, Resource and clock owners over controlled native geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { readReorderLandingPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint-lease.ts';
import { readCurrentReorderGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-current-geometry.ts';
import { reorderHeldMarker } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';

test('whole endpoint retains native static/carrier paint after clock settlement with exact issued mask and no RAF', () => {
  const f = wholeFixture();
  try {
    const anchor = f.root,
      children = [...anchor.childNodes],
      selection = f.state.selection;
    f.prepare();
    const capture = f.jobs.shift();
    assert.ok(capture);
    capture();
    f.microtasks();
    const pending = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(pending);
    const carriers = [
      pending,
      pending.querySelector('.cm-content'),
      pending.querySelector('.cm-line'),
    ];
    for (const carrier of carriers) {
      assert.ok(carrier);
      assert.equal(carrier.style.visibility, 'visible');
      Object.defineProperty(carrier.style, 'visibility', {
        configurable: true,
        get: () => 'visible',
        /** Whole rectangular clipping suppress retained carriers without inherited visibility mutations. */
        set() {
          throw new Error('whole-carrier-visibility-write');
        },
      });
    }
    const zeroClip = 'rect(0px, 0px, 0px, 0px)';
    assert.equal(pending.style.getPropertyPriority('clip'), 'important');
    assert.equal(pending.style.clip, zeroClip);
    assert.equal(pending.dataset.reorderMotionEndpoint, undefined);
    assert.equal(pending.dataset.reorderMotionProgress, undefined);
    assert.equal(pending.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    f.deliver();
    const mask = f.state.field(reorderPreview).mask;
    assert.equal(mask?.coverage, 'value');
    const scene = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(scene);
    assert.equal(scene.dataset.reorderMotionEndpoint, reorderPaintIssueName(mask!.issue));
    assert.equal(scene.style.clip, 'auto');
    const paintedRoots = scene.querySelectorAll('[data-reorder-motion-chip]');
    assert.ok(paintedRoots.length > 0);
    for (const root of paintedRoots) {
      assert.equal(root.style.visibility, 'visible');
      assert.equal(root.style.position, 'relative');
    }
    assert.equal(
      f.state.field(reorderPreview).paint === f.state.field(reorderPreview).pointer!.anchor,
      true,
    );
    f.step(200);
    assert.equal(f.frames.size, 0);
    assert.equal(scene.isConnected, true);
    assert.equal(scene.style.clip, 'auto');
    assert.equal(scene.dataset.reorderMotionProgress, '1');
    assert.equal(f.state.field(reorderPreview).mask === mask, true);
    assert.equal(
      children.length > 0 &&
        anchor.childNodes.length === children.length &&
        children.every((node, at) => anchor.childNodes[at] === node),
      true,
    );
    const display = f.presentation.displayPaint(f.input);
    assert.equal(
      display !== null && display.rects.length > 0 && display.geometry.length >= 3,
      true,
    );
    assert.equal(
      display!.rects.every((rect) => rect.index === 0),
      true,
    );
    assert.equal(f.state.selection === selection, true);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
    f.presentation.cancel();
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
    assert.equal(scene.style.getPropertyPriority('clip'), 'important');
    assert.equal(scene.dataset.reorderMotionEndpoint === undefined, true);
    assert.equal(scene.style.clip, zeroClip);
    assert.equal(scene.dataset.reorderMotionProgress, undefined);
    assert.equal(scene.querySelectorAll('[data-reorder-motion-chip]').length, 0);
    assert.equal(
      carriers.every((carrier) => carrier!.style.visibility === 'visible'),
      true,
    );
    for (const root of paintedRoots) {
      assert.equal(root.style.visibility, '');
      assert.equal(root.style.position, '');
      assert.equal(root.style.left, '');
      assert.equal(root.style.top, '');
    }
    assert.equal(scene.isConnected, false);
  } finally {
    f.destroy();
  }
});

test('delayed whole adoption renews the missing landing READ and contour without another pointer event', () => {
  const f = wholeFixture();
  try {
    let delivered = 0;
    f.onRefresh((reason) => {
      assert.equal(reason, 'endpoint');
      f.view.requestMeasure({
        read: () => {
          const display = f.presentation.displayPaint(f.input);
          assert.equal(display !== null && display.current(), true);
          return display;
        },
        write: (display) =>
          queueMicrotask(() => {
            assert.equal(display!.current(), true);
            f.presentation.mark(f.input, display!.rects, 'rgb(1, 2, 3)', display!.viewport);
            delivered++;
          }),
      });
    });
    assert.equal(
      f.paint.document.body!.querySelector('[data-reorder-landing-contour]') === null,
      true,
    );
    f.prepare();
    f.deliver();
    assert.equal(f.state.field(reorderPreview).mask?.coverage, 'value');
    assert.equal(delivered, 0);
    f.deliver();
    assert.equal(delivered, 1);
    assert.equal(
      f.paint.document.body!.querySelector('[data-reorder-landing-contour]') !== null,
      true,
    );
    assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('changed native block extent refuses whole coverage and publishes current canonical order before revealing native', () => {
  const f = wholeFixture();
  try {
    const create = f.paint.document.createElement.bind(f.paint.document);
    f.paint.document.createElement = (name) => {
      const node = create(name);
      // The controlled native CSS border-box differs; the real preparation must reject it exactly.
      if (name === 'div') Object.assign(node.box, { height: 21 });
      return node;
    };
    f.prepare();
    f.deliver();
    const state = f.state.field(reorderPreview);
    assert.equal(state.mask === null, true);
    assert.equal(state.pointer!.mode, 'fallback');
    assert.equal(state.paint === state.value, true);
    assert.equal(f.frames.size, 0);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('refused whole endpoint renews actual canonical marker geometry without another pointer event', () => {
  const f = wholeFixture();
  try {
    const hidden = hideReorderChip(f.input.capture.prepared, f.input.order, f.input.held);
    assert.ok(hidden);
    Object.assign(f.input, { hidden });
    const selection = f.state.selection,
      create = f.paint.document.createElement.bind(f.paint.document);
    let marker: ReturnType<typeof reorderHeldMarker> = null;
    f.paint.document.createElement = (name) => {
      const node = create(name);
      if (name === 'div') Object.assign(node.box, { height: 21 });
      return node;
    };
    f.onRefresh((reason) => {
      assert.equal(reason, 'endpoint');
      const publication = f.state.field(reorderPreview);
      assert.equal(publication.pointer!.mode, 'fallback');
      assert.equal(publication.paint, publication.value);
      assert.equal(publication.mask, null);
      assert.equal(f.paint.editor.querySelector('.sp-reorder-motion-stage'), null);
      f.view.requestMeasure({
        read: () =>
          readReorderLandingPaint(f.view, publication.value, null, (current, preview) => ({
            current,
            preview,
            rect: reorderHeldMarker(
              f.input.order,
              hidden,
              f.input.held,
              readCurrentReorderGeometry(f.view, {
                kind: 'marker',
                input: f.input,
              }),
              f.view.scrollDOM.getBoundingClientRect(),
            ),
          })),
        write: (measured) =>
          queueMicrotask(() => {
            assert.ok(measured);
            assert.ok(measured.current());
            assert.equal(f.state.field(reorderPreview).value, measured.preview);
            marker = measured.rect;
          }),
      });
    });
    f.prepare();
    f.deliver();
    f.deliver();
    assert.ok(marker, 'fallback must issue a fresh canonical marker READ');
    assert.equal(f.refreshes, 1);
    assert.equal(f.frames.size, 0);
    assert.equal(f.jobs.length + f.tasks.length, 0);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const boundary of ['dispose', 'publication', 'request'] as const)
  test(`fallback settlement rejects an unavailable handoff superseded by ${boundary}`, () => {
    const f = wholeFixture();
    try {
      const create = f.paint.document.createElement.bind(f.paint.document);
      f.paint.document.createElement = (name) => {
        const node = create(name);
        if (name === 'div') Object.assign(node.box, { height: 21 });
        return node;
      };
      f.presentation.mark(
        f.input,
        f.origin,
        'rgb(1,2,3)',
        f.view.scrollDOM.getBoundingClientRect(),
      );
      const landing = f.paint.document.body!.querySelector('.sp-reorder-landing');
      assert.ok(landing);
      const contour = landing.querySelector('[data-reorder-landing-contour]');
      assert.ok(contour);
      assert.equal(contour.getAttribute('stroke'), 'rgb(1,2,3)');
      contour.setAttribute('stroke', 'rgb(4,5,6)');
      assert.equal(contour.getAttribute('stroke'), 'rgb(4,5,6)');
      const setAttribute = contour.setAttribute.bind(contour),
        replacement = { retained: () => false };
      let reentered = false;
      let fallbackChanged = false,
        fallbackRequested = false;
      // The last native paint setter can synchronously replace the request before unavailable continues.
      contour.setAttribute = (name, value) => {
        const previousStroke = contour.getAttribute('stroke');
        setAttribute(name, value);
        if (name === 'stroke' && f.state.field(reorderPreview).pointer?.mode !== 'fallback')
          setAttribute(name, 'rgb(4,5,6)');
        if (
          reentered ||
          name !== 'stroke' ||
          f.state.field(reorderPreview).pointer?.mode !== 'fallback'
        )
          return;
        fallbackChanged = previousStroke !== value;
        fallbackRequested = value === 'rgb(1,2,3)';
        reentered = true;
        if (boundary === 'dispose') f.presentation.destroy();
        else if (boundary === 'publication') f.publish();
        else {
          f.prepare();
          replacement.retained = f.presentation.retain(f.input) ?? (() => false);
        }
      };
      f.prepare();
      for (let phase = 0; phase < 4 && !reentered && f.jobs.length; phase++) {
        f.jobs.shift()!();
        f.microtasks();
      }
      assert.equal(reentered, true);
      assert.equal(fallbackChanged, true);
      assert.equal(fallbackRequested, true);
      assert.equal(f.refreshes, 0);
      if (boundary === 'request') {
        assert.equal(f.jobs.length, 1);
        assert.equal(replacement.retained(), true);
        f.deliver();
      }
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

for (const cause of [null, undefined])
  test(`fallback endpoint refresh preserves raw ${String(cause)} after exact Stage retirement`, () => {
    const f = wholeFixture();
    try {
      const create = f.paint.document.createElement.bind(f.paint.document);
      f.paint.document.createElement = (name) => {
        const node = create(name);
        if (name === 'div') Object.assign(node.box, { height: 21 });
        return node;
      };
      f.onRefresh((reason) => {
        assert.equal(reason, 'endpoint');
        assert.equal(f.state.field(reorderPreview).pointer!.mode, 'fallback');
        assert.equal(f.paint.editor.querySelector('.sp-reorder-motion-stage'), null);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      });
      f.prepare();
      assert.throws(
        () => f.deliver(),
        (error) => error === cause,
      );
      assert.equal(f.refreshes, 1);
      assert.equal(f.state.field(reorderPreview).mask, null);
      assert.equal(f.frames.size, 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('retired whole observation rejects its old issue after native connectivity reentry', () => {
  const f = wholeFixture();
  let scene: ReturnType<typeof f.paint.editor.querySelector> = null;
  try {
    f.prepare();
    f.deliver();
    scene = f.paint.editor.querySelector('[data-reorder-motion-endpoint]');
    assert.ok(scene);
    assert.equal(Object.hasOwn(scene, 'isConnected'), false);
    const node = scene;
    let intercepted = false;
    Object.defineProperty(node, 'isConnected', {
      configurable: true,
      /** Preserve the genuine native result while retiring the exact issue during its physical lease check. */
      get() {
        const connected: unknown = Reflect.get(
          Object.getPrototypeOf(node) as object,
          'isConnected',
          node,
        );
        assert.equal(typeof connected, 'boolean');
        if (!intercepted) {
          intercepted = true;
          f.presentation.cancel();
        }
        return connected;
      },
    });
    assert.equal(f.presentation.displayPaint(f.input) === null, true);
    assert.equal(intercepted, true);
    assert.equal(f.frames.size, 0);
  } finally {
    if (scene) Reflect.deleteProperty(scene, 'isConnected');
    f.destroy();
  }
});
