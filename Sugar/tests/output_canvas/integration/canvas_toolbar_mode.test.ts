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
/** Prove Plugin Mode removes and restores the same output canvas affordance. */
import { expect, jest, test } from '@jest/globals';
import { ComfyCanvasToolbar } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasToolbar.js';
import { publishPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';

test('restores toolbar availability even when the saved canvas visibility remains closed', () => {
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const slot = document.createElement('div');
  const queue = document.createElement('button');
  queue.dataset.testid = 'queue-overlay-toggle';
  slot.append(queue);
  bar.append(slot);
  document.body.append(bar);
  const toggle = jest.fn();
  const toolbar = new ComfyCanvasToolbar(document, toggle);
  toolbar.setup();
  toolbar.update(false, true);
  const button = bar.querySelector<HTMLButtonElement>('.sc-output-canvas-toggle');
  expect(button?.disabled).toBe(false);
  publishPluginMode(document, true);
  expect(button?.hidden).toBe(true);
  expect(button?.disabled).toBe(true);
  toolbar.update(false, true);
  publishPluginMode(document, false);
  expect(button?.hidden).toBe(false);
  expect(button?.disabled).toBe(false);
  button?.click();
  expect(toggle).toHaveBeenCalledTimes(1);
  expect(bar.querySelector('.sc-output-canvas-toggle')).toBe(button);
  toolbar.update(false, false);
  publishPluginMode(document, true);
  publishPluginMode(document, false);
  expect(button?.disabled).toBe(true);
  toolbar.dispose();
  bar.remove();
});
