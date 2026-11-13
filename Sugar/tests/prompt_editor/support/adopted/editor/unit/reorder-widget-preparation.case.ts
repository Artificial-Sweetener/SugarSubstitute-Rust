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
/** Canonical preparation failures retire their cold mount without losing raw causes or retaining unpublished resources. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

for (const expected of [null, undefined])
  test(`cold classification preserves a raw ${expected === null ? 'null' : 'undefined'} failure and retires its mount once`, (context) => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document'),
      failures: unknown[] = [];
    let fixture: ReturnType<typeof motionPresentationFixture> | null = null;
    let restore = (): void => {};
    try {
      fixture = motionPresentationFixture(
        preparedLoraState(() => {}, 2, 16, 8),
        (document) =>
          Object.defineProperty(globalThis, 'document', {
            configurable: true,
            value: document,
          }),
      );
      const capture = fixture.input.capture,
        order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
          kind: 'row',
          row: 0,
          insertion: 1,
        });
      assert.ok(order);
      const preview = prepareReorderPreview(
        {
          capture,
          token: fixture.input.token,
          activeChip: 0,
          order,
        },
        'complete',
      );
      assert.ok(preview);
      const ordinary = capture.projection!.ordinary,
        own = Object.getOwnPropertyDescriptor(ordinary, 'between'),
        widget = new ReorderWidget(preview, null),
        layout = context.mock.method(ReorderMountedLayout.prototype, 'destroy');
      restore = () => {
        if (own) Object.defineProperty(ordinary, 'between', own);
        else Reflect.deleteProperty(ordinary, 'between');
      };
      Object.defineProperty(ordinary, 'between', {
        configurable: true,
        /** A real classification read fails before any cold fragment can be materialized. */
        value(): never {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw expected;
        },
      });
      let observed: unknown,
        threw = false;
      try {
        widget.mountPassive(fixture.view);
      } catch (cause) {
        threw = true;
        observed = cause;
      }
      assert.equal(threw, true);
      assert.equal(observed, expected);
      assert.equal(layout.mock.callCount(), 1);
      const root = fixture.created.at(-1);
      assert.ok(root);
      assert.equal(root.className, 'sp-reorder-preview');
      assert.equal(root.childNodes.length, 0);
      widget.destroy(root as unknown as HTMLElement);
      assert.equal(layout.mock.callCount(), 1);
    } catch (cause) {
      failures.push(cause);
    }
    for (const release of [
      restore,
      () => fixture?.destroy(),
      () => {
        if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'cold-classification-proof-retirement');
  });
