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
/** Verify graph navigation publishes changes without a timer. */

import { jest } from '@jest/globals';
import { ComfyCanvasGraphChangeAdapter } from '../../../frontend/comfyui/ui/surface/ComfyCanvasGraphChangeAdapter.js';

test('publishes host and extension graph transitions and restores Comfy ownership', () => {
  const root = { id: 'root' };
  const nested = { id: 'nested' };
  const originalSetGraph = jest.fn(function (this: { graph: object | null }, graph: object): void {
    this.graph = graph;
  });
  const canvas = {
    graph: null as object | null,
    setGraph: originalSetGraph,
  };
  const adapter = new ComfyCanvasGraphChangeAdapter(canvas);
  const changed = jest.fn();
  adapter.subscribe(changed);

  canvas.setGraph(root);
  adapter.setGraph(nested);

  expect(canvas.graph).toBe(nested);
  expect(originalSetGraph).toHaveBeenCalledTimes(2);
  expect(changed).toHaveBeenCalledTimes(2);
  adapter.dispose();
  expect(canvas.setGraph).toBe(originalSetGraph);

  canvas.setGraph(root);
  expect(changed).toHaveBeenCalledTimes(2);
});

test('does not publish a failed host graph transition', () => {
  const failure = new Error('navigation failed');
  const canvas = {
    setGraph: jest.fn((_graph: object) => {
      throw failure;
    }),
  };
  const adapter = new ComfyCanvasGraphChangeAdapter(canvas);
  const changed = jest.fn();
  adapter.subscribe(changed);

  expect(() => canvas.setGraph({ id: 'nested' })).toThrow(failure);
  expect(changed).not.toHaveBeenCalled();
  adapter.dispose();
});
