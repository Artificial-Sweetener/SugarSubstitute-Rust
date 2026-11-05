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
/** Prove the scene combo box layer of the canvas navigation bar. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import type { CanvasViewSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => document.body.replaceChildren());

/** A scene run viewed on All: scenes are published and source tabs are not. */
function sceneSnapshot(): CanvasViewSnapshot {
  const snapshot = canvasSnapshot('scenes');
  return {
    ...snapshot,
    view: { ...snapshot.view, mode: 'overview', scene: null },
    sources: [],
    scenes: [
      { key: 'morning', title: 'Morning', available: true },
      { key: 'night', title: 'Night', available: false },
    ],
  };
}

test('the scene picker offers All first and disables scenes without images', async () => {
  const port = new CanvasPortFixture(sceneSnapshot(), ['select-scene', 'select-source', 'batch']);
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  expect(root.querySelectorAll('.sc-output-tabstrip')).toHaveLength(1);
  expect(root.querySelectorAll('[role="tab"]')).toHaveLength(0);
  const picker = root.querySelector<HTMLButtonElement>('button[aria-label="Output scene"]')!;
  expect(picker.textContent).toBe('All');
  expect(picker.querySelector('.sc-output-chevron')).not.toBeNull();
  picker.click();
  await nextTick();
  await nextTick();
  const rows = [...document.querySelectorAll<HTMLButtonElement>('[role="option"]')];
  expect(rows.map((row) => [row.textContent, row.getAttribute('aria-disabled')])).toEqual([
    ['All', null],
    ['Morning', null],
    ['Night', 'true'],
  ]);
  rows[2]!.click();
  expect(port.requests).toHaveLength(0);
  rows[1]!.click();
  await Promise.resolve();
  expect(port.requests.at(-1)?.command).toEqual({ type: 'select-scene', scene: 'morning' });
  mounted.dispose();
});

test('a workflow without several scenes has no scene picker', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('plain')),
    jest.fn(),
  );
  expect(root.querySelector('button[aria-label="Output scene"]')).toBeNull();
  mounted.dispose();
});
