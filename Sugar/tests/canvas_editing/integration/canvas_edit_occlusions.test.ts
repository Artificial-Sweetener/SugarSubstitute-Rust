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
/** Prove the native canvas yields pointer input under every editing surface. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { NativeCanvasOcclusions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasOcclusions.js';
import { editState } from '../support/CanvasEditHostFixture.js';
import {
  disposeEditing,
  clientRect,
  mountEditing,
  VIEWPORT,
} from '../support/CanvasEditOverlayFixture.js';

afterEach(() => {
  disposeEditing();
  document.body.replaceChildren();
  jest.restoreAllMocks();
  Reflect.deleteProperty(document, 'elementsFromPoint');
});

/** Lay out chrome by class, and the viewport and its ancestors over the whole canvas. */
function layout(viewport: HTMLElement): void {
  const boxes: [string, { left: number; top: number; width: number; height: number }][] = [
    ['.sc-canvas-edit-rail', { left: 108, top: 102, width: 40, height: 260 }],
    [
      '.sc-canvas-edit-topbar > .sc-canvas-edit-chrome',
      { left: 108, top: 58, width: 64, height: 36 },
    ],
    ['.sc-canvas-edit-toolbar', { left: 400, top: 362, width: 200, height: 36 }],
    ['.sc-canvas-edit-coverage', { left: 300, top: 600, width: 400, height: 42 }],
  ];
  jest.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: Element,
  ) {
    const box = boxes.find(([selector]) => this.matches(selector))?.[1];
    if (box) return clientRect(box);
    return this === viewport || this.contains(viewport)
      ? clientRect(VIEWPORT)
      : clientRect({ left: 0, top: 0, width: 0, height: 0 });
  });
  Object.defineProperty(document, 'elementsFromPoint', { configurable: true, value: () => [] });
}

const measure = (occlusions: NativeCanvasOcclusions) =>
  occlusions.read(
    { x: VIEWPORT.left, y: VIEWPORT.top },
    { width: VIEWPORT.width, height: VIEWPORT.height },
  );

describe('editing chrome occlusions', () => {
  test('reports the rail, history and contextual toolbar as covering the canvas', async () => {
    const f = await mountEditing(
      editState({ selection: { bounds: { left: 300, top: 100, width: 200, height: 200 } } }),
    );
    layout(f.viewport);
    const occlusions = new NativeCanvasOcclusions(f.scope, f.viewport);
    expect(measure(occlusions)).toEqual({
      occlusions: expect.arrayContaining([
        { x: 8, y: 52, width: 40, height: 260 },
        { x: 8, y: 8, width: 64, height: 36 },
        { x: 300, y: 312, width: 200, height: 36 },
      ]),
      modal: false,
    });
    expect(measure(occlusions).occlusions).toHaveLength(3);
  });

  test('blocks the whole canvas while the coverage editor is open', async () => {
    const f = await mountEditing(editState({ mode: 'coverage' }));
    layout(f.viewport);
    const measured = measure(new NativeCanvasOcclusions(f.scope, f.viewport));
    expect(measured.modal).toBe(true);
    expect(measured.occlusions).toEqual([{ x: 200, y: 550, width: 400, height: 42 }]);
  });
});
