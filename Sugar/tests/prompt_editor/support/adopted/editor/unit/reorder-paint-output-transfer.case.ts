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
import { captureNativePaint } from './reorder-native-paint-fixture.ts';
/** Successful native scene/grab roots remain owned until request-local template retirement permits transfer. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { captureReorderMotionGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
import { captureReorderGrabPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-paint.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import type { EditorView } from '@codemirror/view';

for (const kind of ['native-paint', 'cold-grab', 'paused-grab'] as const)
  for (const outputFails of [false, true])
    test(`${kind} successful root rolls back after template retirement failure; output failure=${outputFails}`, () => {
      const f = motionPaintFixture(),
        templateFailure = new Error('controlled-template-retirement'),
        outputFailure = new Error('controlled-output-retirement');
      const baselineSource = f.line.textContent,
        create = f.document.createElement.bind(f.document);
      let templateRemovals = 0,
        outputRemovals = 0,
        output: typeof f.line | null = null;
      const originalScene = kind === 'paused-grab' ? captureNativePaint(f) : null;
      if (kind === 'paused-grab') {
        assert.ok(originalScene);
        // Production paused Scenes are connected before their native geometry is read.
        assert.ok(f.document.body);
        f.document.body.append(originalScene as unknown as typeof f.line);
      }
      const geometry = originalScene
        ? reorderGrabGeometry(
            [
              {
                kind: 'chip',
                index: 2,
                left: 100,
                right: 120,
                top: 20,
                bottom: 40,
              },
            ],
            2,
            { x: 100, y: 25 },
          )
        : null;
      if (originalScene) assert.ok(geometry);
      f.document.createElement = (tag) => {
        const node = create(tag),
          remove = node.remove.bind(node);
        node.remove = () => {
          remove();
          if (node.className === '') {
            if (templateRemovals++ === 0) throw templateFailure;
          } else if (
            node.className.includes(kind === 'native-paint' ? 'cm-line' : 'sp-reorder-proxy')
          ) {
            assert.ok(
              node.childNodes.length,
              'actual completed parent must contain prepared copies before failed transfer',
            );
            assert.ok(node.childNodes.every((child) => child.parent === node));
            output = node;
            ++outputRemovals;
            if (outputFails) throw outputFailure;
          }
        };
        return node;
      };
      let failed = false,
        actual: unknown;
      try {
        if (kind === 'native-paint') captureNativePaint(f);
        else if (originalScene && geometry)
          captureReorderMotionGrab(f.view, { node: originalScene }, geometry);
        else {
          const view = {
            // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
            ...f.view,
            contentDOM: {
              querySelectorAll: (selector: string) =>
                selector.startsWith('[data-reorder-chip') ? [f.chips[0]] : [f.line],
              contains: (line: unknown) => line === f.line,
            },
          } as unknown as EditorView;
          captureReorderGrabPaint(view, 0, { x: 20, y: 25 });
        }
      } catch (cause) {
        failed = true;
        actual = cause;
      } finally {
        originalScene?.remove();
      }
      assert.equal(failed, true);
      if (outputFails) {
        assert.ok(actual instanceof AggregateError);
        assert.deepEqual(actual.errors, [templateFailure, outputFailure]);
      } else assert.equal(actual, templateFailure);
      assert.ok(templateRemovals > 0);
      assert.equal(outputRemovals, 1);
      assert.ok(output);
      assert.equal((output as typeof f.line).removed, true);
      assert.equal(f.line.textContent, baselineSource);
      assert.equal(f.frames.size, 0);
    });

for (const failureAt of ['structural-classes', 'proxy-allocation'] as const)
  for (const primary of [null, undefined, new Error('controlled-paused-grab-adaptation')] as const)
    for (const cleanupFails of [false, true])
      test(`paused grab ${failureAt} retains untransferred copy and raw primary ${String(primary)} cleanup=${cleanupFails}`, () => {
        const f = motionPaintFixture(),
          original = captureNativePaint(f),
          cleanup = new Error('controlled-untransferred-copy-retirement');
        assert.ok(original);
        assert.ok(f.document.body);
        f.document.body.append(original as unknown as typeof f.line);
        const geometry = reorderGrabGeometry(
          [
            {
              kind: 'chip',
              index: 1,
              left: 60,
              right: 80,
              top: 20,
              bottom: 40,
            },
          ],
          1,
          { x: 65, y: 25 },
        );
        assert.ok(geometry);
        const create = f.document.createElement.bind(f.document);
        let stripped = false,
          removals = 0;
        f.document.createElement = (name) => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          if (stripped && failureAt === 'proxy-allocation') throw primary;
          const node = create(name),
            query = node.querySelectorAll.bind(node),
            remove = node.remove.bind(node);
          node.querySelectorAll = (selector) => {
            if (selector === '.cm-content, .cm-scroller') {
              stripped = true;
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              if (failureAt === 'structural-classes') throw primary;
            }
            return query(selector);
          };
          node.remove = () => {
            remove();
            if (node.className === 'cm-line') {
              removals++;
              if (cleanupFails) throw cleanup;
            }
          };
          return node;
        };
        try {
          let failed = false,
            actual: unknown;
          try {
            captureReorderMotionGrab(f.view, { node: original }, geometry);
          } catch (cause) {
            failed = true;
            actual = cause;
          }
          assert.equal(failed, true);
          if (cleanupFails) {
            assert.ok(actual instanceof AggregateError);
            assert.deepEqual(actual.errors, [primary, cleanup]);
          } else assert.equal(actual, primary);
          assert.equal(removals, 1);
          assert.equal(f.line.textContent, 'abc');
          assert.equal(f.frames.size, 0);
        } finally {
          original.remove();
        }
      });
