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
import { atomicPaintFixture } from './reorder-atomic-paint-fixture.ts';
/** Native external styles/rectangles are controlled; real canonical LoRA constructors and the grab painter own actual copied output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { captureReorderGrabPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-paint.ts';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { prepareNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { projectionState } from '../transaction/projection-state.ts';

for (const position of ['static', 'relative'] as const) {
  test(`${position} admitted 45-canvas canonical line copies only the complete held wrapper and its exact borrowed native pixels`, () => {
    const f = atomicPaintFixture(position);
    let paint: ReturnType<typeof captureReorderGrabPaint> = null;
    try {
      assert.ok(admitNativeReorderPaint([f.f.line as unknown as HTMLElement]));
      assert.equal(f.canvases.length, 45);
      const source = readExactSource(f.view.state),
        state = f.view.state;
      paint = captureReorderGrabPaint(f.view, 0, { x: 29.25, y: 34 });
      assert.ok(paint);
      assert.equal(paint.node.childNodes.length, 1);
      const wrapper = paint.node.childNodes[0] as HTMLElement;
      assert.equal(wrapper.className, 'sp-reorder-chip');
      assert.equal(wrapper.textContent, f.roots[0]!.textContent);
      assert.equal(wrapper.querySelectorAll('canvas').length, 1);
      assert.equal(wrapper.querySelectorAll('.sp-lora-weight').length, 1);
      assert.equal(f.draws.length, 1);
      assert.equal(f.draws[0]!.source, f.canvases[0]);
      assert.notEqual(f.draws[0]!.destination, f.canvases[0]);
      const copied = f.draws[0]!.destination as HTMLCanvasElement;
      assert.deepEqual([copied.width, copied.height], [256, 256]);
      assert.deepEqual([paint.geometry.grabX, paint.geometry.grabY], [9.25, 14]);
      assert.equal((wrapper as HTMLElement).style.getPropertyValue('border-radius'), '9px');
      assert.equal(readExactSource(f.view.state), source);
      assert.equal(f.view.state, state);
      assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
      assert.ok(f.roots.every((root) => !root.removed));
    } catch (cause) {
      f.causes.push(cause);
    }
    try {
      paint?.node.remove();
    } catch (cause) {
      f.causes.push(cause);
    }
    f.destroy();
  });
  for (const mode of [
    'outside-text',
    'outside-space',
    'root-transform',
    'root-position',
    'root-fixed',
    'relative-top',
    'relative-right',
    'relative-bottom',
    'relative-left',
    'relative-auto',
    'relative-other-unit',
    'relative-unknown',
    'outside-generated',
    'second-island',
  ] as const)
    test(`${position} canonical ${mode} retains complete-line output without admitting a guessed shaping island`, () => {
      const f = atomicPaintFixture(position);
      let paint: ReturnType<typeof captureReorderGrabPaint> = null;
      try {
        const root = f.roots[0]!;
        if (mode === 'outside-text' || mode === 'outside-space')
          root.append(f.f.text(mode === 'outside-text' ? 'العربية' : ' '));
        if (mode === 'second-island') {
          const island = f.f.document.createElement('span');
          island.className = 'sp-lora-chip';
          island.append(f.f.text('other'));
          root.append(island);
        }
        const read = f.f.document.defaultView.getComputedStyle;
        f.f.document.defaultView.getComputedStyle = (node, pseudo?: string) => {
          const value = read(node);
          return node !== root
            ? value
            : {
                ...value,
                content:
                  mode === 'outside-generated' && pseudo
                    ? '"generated"'
                    : value.getPropertyValue('content'),
                transform:
                  mode === 'root-transform'
                    ? 'matrix(1, 0, 0, 1, 4, 0)'
                    : value.getPropertyValue('transform'),
                position:
                  mode === 'root-position'
                    ? 'absolute'
                    : mode === 'root-fixed'
                      ? 'fixed'
                      : mode.startsWith('relative-')
                        ? 'relative'
                        : value.getPropertyValue('position'),
                inset:
                  mode === 'relative-top'
                    ? '4px 0px 0px'
                    : mode === 'relative-right'
                      ? '0px 4px 0px 0px'
                      : mode === 'relative-bottom'
                        ? '0px 0px 4px'
                        : mode === 'relative-left'
                          ? '0px 0px 0px 4px'
                          : mode === 'relative-unknown'
                            ? ''
                            : mode === 'relative-auto'
                              ? 'auto'
                              : mode === 'relative-other-unit'
                                ? '0%'
                                : '0px',
              };
        };
        paint = captureReorderGrabPaint(f.view, 0, { x: 29.25, y: 34 });
        assert.ok(paint);
        assert.equal((paint.node.childNodes[0] as HTMLElement).className, 'cm-line');
        assert.equal(paint.node.querySelectorAll('canvas').length, 45);
        assert.deepEqual(
          f.draws.map((draw) => draw.source),
          f.canvases,
        );
        assert.ok(f.roots.every((node) => !node.removed));
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        paint?.node.remove();
      } catch (cause) {
        f.causes.push(cause);
      }
      f.destroy();
    });

  for (const mode of ['unavailable', 'raw-null', 'raw-undefined', 'raw-error'] as const)
    test(`${position} preferred ${mode} materialization is terminal and never retries the surrounding line`, () => {
      const f = atomicPaintFixture(position),
        create = f.f.document.createElement.bind(f.f.document),
        primary = new Error('controlled-atomic-draw');
      let attempts = 0,
        paint: ReturnType<typeof captureReorderGrabPaint> = null;
      try {
        f.f.document.createElement = (tag) => {
          const node = create(tag);
          if (tag === 'canvas')
            Object.assign(node, {
              getContext: () => {
                ++attempts;
                if (mode === 'unavailable') return null;
                return {
                  drawImage() {
                    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
                    throw mode === 'raw-null'
                      ? null
                      : mode === 'raw-undefined'
                        ? undefined
                        : primary;
                  },
                };
              },
            });
          return node;
        };
        if (mode === 'unavailable') {
          paint = captureReorderGrabPaint(f.view, 0, { x: 29.25, y: 34 });
          assert.equal(paint, null);
        } else {
          assert.throws(
            () => captureReorderGrabPaint(f.view, 0, { x: 29.25, y: 34 }),
            (cause: unknown) =>
              cause ===
              (mode === 'raw-null' ? null : mode === 'raw-undefined' ? undefined : primary),
          );
        }
        assert.equal(attempts, 1);
        assert.ok(f.roots.every((node) => !node.removed));
      } catch (cause) {
        f.causes.push(cause);
      }
      try {
        paint?.node.remove();
      } catch (cause) {
        f.causes.push(cause);
      }
      f.destroy();
    });
}

for (const mode of [
  'raw-null',
  'raw-undefined',
  'raw-error',
  'source-supersede',
  'root-detach',
] as const)
  test(`relative inset preparation preserves ${mode} native getter boundary`, () => {
    const f = atomicPaintFixture(),
      primary = new Error('controlled-relative-inset');
    try {
      const root = f.roots[0]!,
        dom = root as unknown as HTMLElement,
        nodes = admitNativeReorderPaint([dom]);
      assert.ok(nodes);
      const read = f.f.document.defaultView.getComputedStyle;
      f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
        const value = read(node, pseudo);
        return node !== root
          ? value
          : {
              ...value,
              get inset() {
                // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
                if (mode === 'raw-null') throw null;
                // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
                if (mode === 'raw-undefined') throw undefined;
                if (mode === 'raw-error') throw primary;
                if (mode === 'root-detach') root.remove();
                else
                  Object.defineProperty(f.view, 'state', {
                    value: projectionState('tail'),
                  });
                return '0px';
              },
            };
      };
      if (mode === 'source-supersede' || mode === 'root-detach')
        assert.equal(prepareNativeReorderPaint(f.view, [dom], nodes, 'atomic-grab'), null);
      else
        assert.throws(
          () => prepareNativeReorderPaint(f.view, [dom], nodes, 'atomic-grab'),
          (cause: unknown) =>
            cause === (mode === 'raw-null' ? null : mode === 'raw-undefined' ? undefined : primary),
        );
      assert.equal(f.draws.length, 0);
    } catch (cause) {
      f.causes.push(cause);
    }
    f.destroy();
  });
