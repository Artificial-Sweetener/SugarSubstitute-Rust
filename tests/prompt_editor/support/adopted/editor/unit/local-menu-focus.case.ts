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
/** Actual surface capabilities characterize pointer publication reentry; browser proof owns trusted routing and physical focus. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { contextMenuTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
import { openLocalPointerMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-local-presentation.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';
import { nativeSurfaceFixture } from './native-surface-fixture.ts';

/** The inert mouse boundary transports only viewport CSS pixels and default prevention; it owns no native input or source semantics. */
function pointer(prevented: () => void): MouseEvent {
  return {
    clientX: 20,
    clientY: 30,
    preventDefault: prevented,
  } as MouseEvent;
}

for (const replacement of ['destination', 'new-intent'] as const)
  test(`accepted pointer opening preserves a callback-selected ${replacement}`, async () => {
    const native = nativeSurfaceFixture(),
      surface = native.surface,
      original = {},
      destination = {};
    native.root.activeElement = original;
    const before = surface.view.state,
      target = contextMenuTargetAt(before, 0);
    assert.ok(target);
    let newer: FocusHandoff | undefined,
      prevented = 0;
    try {
      assert.equal(
        openLocalPointerMenu(
          surface.view,
          target,
          pointer(() => prevented++),
          (captured, point) => {
            assert.equal(captured, target);
            assert.deepEqual(point, { x: 20, y: 30 });
            native.root.activeElement = destination;
            if (replacement === 'new-intent') newer = surface.handoff();
            return true;
          },
        ),
        true,
      );
      await native.flush();
      assert.equal(prevented, 1);
      assert.equal(native.root.activeElement, destination);
      assert.equal(native.counts().focuses, 0);
      assert.equal(surface.view.state, before);
      assert.deepEqual([undoDepth(before), redoDepth(before)], [1, 0]);
      if (newer) assert.equal(surface.currentHandoff(newer), true);
    } finally {
      newer?.cancel();
      surface.dispose();
    }
  });

test('accepted pointer focus is synchronous and preserves the exact native state and scroll intent', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  native.root.activeElement = native.external;
  const before = surface.view.state,
    target = contextMenuTargetAt(before, 0);
  assert.ok(target);
  try {
    assert.equal(
      openLocalPointerMenu(
        surface.view,
        target,
        pointer(() => undefined),
        () => {
          assert.equal(surface.view.hasFocus, false);
          return true;
        },
      ),
      true,
    );
    assert.equal(surface.view.hasFocus, true);
    await native.flush();
    assert.equal(native.counts().focuses, 1);
    assert.equal(surface.view.state, before);
    assert.deepEqual([undoDepth(before), redoDepth(before)], [1, 0]);
  } finally {
    surface.dispose();
  }
});

for (const failure of [false, true])
  test(`pointer ${failure ? 'failure' : 'refusal'} retires its original capture without revoking newer intent`, () => {
    const native = nativeSurfaceFixture(),
      surface = native.surface;
    native.root.activeElement = native.external;
    const before = surface.view.state,
      target = contextMenuTargetAt(before, 0),
      cause = new Error('pointer-request-failed');
    assert.ok(target);
    let newer: FocusHandoff | undefined,
      prevented = 0;
    const request = (): boolean => {
      newer = surface.handoff();
      if (failure) throw cause;
      return false;
    };
    try {
      const invoke = (): boolean =>
        openLocalPointerMenu(
          surface.view,
          target,
          pointer(() => prevented++),
          request,
        );
      if (failure) assert.throws(invoke, (failureCause) => failureCause === cause);
      else assert.equal(invoke(), false);
      assert.ok(newer);
      assert.equal(surface.currentHandoff(newer), true);
      assert.equal(prevented, 0);
      assert.equal(native.counts().focuses, 0);
      assert.equal(native.root.activeElement, native.external);
      assert.equal(surface.view.state, before);
    } finally {
      newer?.cancel();
      surface.dispose();
    }
  });
