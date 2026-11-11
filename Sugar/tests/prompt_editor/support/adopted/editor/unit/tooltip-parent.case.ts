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
/** Native placement consumes one current document viewport and preserves tiny-axis space independently of editor transforms. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { nativeTooltipSpace } from '../../../../../../frontend/sugarprompt/editor/src/tooltip-parent.ts';

test('all native tooltip consumers share current four-pixel edges and bounded tiny axes', () => {
  const viewport = { clientWidth: 400, clientHeight: 800 };
  const view = {
    dom: { ownerDocument: { documentElement: viewport } },
    scaleX: 3,
    scaleY: 2,
  } as unknown as EditorView;
  assert.deepEqual(nativeTooltipSpace(view), {
    left: 4,
    right: 396,
    top: 4,
    bottom: 796,
  });
  viewport.clientWidth = 8;
  viewport.clientHeight = 9;
  assert.deepEqual(nativeTooltipSpace(view), {
    left: 0,
    right: 8,
    top: 4,
    bottom: 5,
  });
  viewport.clientWidth = 0;
  viewport.clientHeight = 1;
  assert.deepEqual(nativeTooltipSpace(view), {
    left: 0,
    right: 0,
    top: 0,
    bottom: 1,
  });
  Object.defineProperty(viewport, 'clientWidth', {
    get: () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw null;
    },
  });
  let caught = false;
  try {
    nativeTooltipSpace(view);
  } catch (cause) {
    caught = true;
    assert.equal(cause, null);
  }
  assert.equal(caught, true);
});
