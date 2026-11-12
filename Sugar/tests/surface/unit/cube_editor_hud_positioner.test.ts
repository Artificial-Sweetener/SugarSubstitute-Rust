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
/** Verify the Cube editor HUD observer cannot wake itself indefinitely. */

import { jest } from '@jest/globals';
import { CubeEditorHudPositioner } from '../../../frontend/comfyui/ui/surface/CubeEditorHudPositioner.js';

test('does not rewrite unchanged owned style properties after observing itself', async () => {
  document.body.replaceChildren();
  mountViewport();
  const root = document.createElement('aside');
  document.body.append(root);
  const setProperty = jest.spyOn(root.style, 'setProperty');
  const positioner = new CubeEditorHudPositioner(document, root);

  await Promise.resolve();
  await Promise.resolve();
  expect(setProperty).toHaveBeenCalledTimes(4);

  document.body.append(document.createElement('div'));
  await Promise.resolve();
  await Promise.resolve();
  expect(setProperty).toHaveBeenCalledTimes(4);
  positioner.dispose();
});

test('does not rediscover chrome when a widget or popup changes', async () => {
  document.body.replaceChildren();
  mountViewport();
  const root = document.createElement('aside');
  document.body.append(root);
  const positioner = new CubeEditorHudPositioner(document, root);
  await Promise.resolve();
  await Promise.resolve();
  const sync = jest.spyOn(positioner, 'sync');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  popup.style.height = '100px';
  await Promise.resolve();
  await Promise.resolve();
  expect(sync).not.toHaveBeenCalled();
  positioner.dispose();
  jest.restoreAllMocks();
});

test('uses actual graph bounds when the breadcrumb is hidden and follows graph replacement', async () => {
  document.body.replaceChildren();
  const graph = document.createElement('div');
  graph.id = 'graph-canvas-container';
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  graph.append(canvas);
  jest.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
    x: 60,
    y: 86,
    left: 60,
    top: 86,
    right: 660,
    bottom: 886,
    width: 600,
    height: 800,
    toJSON: () => ({}),
  });
  const root = document.createElement('aside');
  document.body.append(graph, root);
  const positioner = new CubeEditorHudPositioner(document, root);
  expect(root.hidden).toBe(false);
  expect(root.style.getPropertyValue('--sugarcubes-cube-editor-metadata-left')).toBe('76px');
  expect(root.style.getPropertyValue('--sugarcubes-cube-editor-metadata-top')).toBe('102px');
  expect(root.style.maxWidth).toBe('568px');
  expect(root.style.maxHeight).toBe('768px');
  graph.remove();
  await Promise.resolve();
  await Promise.resolve();
  expect(root.hidden).toBe(true);
  const replacement = document.createElement('canvas');
  replacement.id = 'graph-canvas';
  jest.spyOn(replacement, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 120,
    left: 0,
    top: 120,
    right: 400,
    bottom: 620,
    width: 400,
    height: 500,
    toJSON: () => ({}),
  });
  graph.replaceChildren(replacement);
  document.body.append(graph);
  await Promise.resolve();
  await Promise.resolve();
  expect(root.hidden).toBe(false);
  expect(root.style.getPropertyValue('--sugarcubes-cube-editor-metadata-top')).toBe('136px');
  expect(root.style.maxWidth).toBe('368px');
  positioner.dispose();
});

test('hides without a measured graph and ignores queued viewport work after disposal', async () => {
  document.body.replaceChildren();
  const root = document.createElement('aside');
  document.body.append(root);
  const positioner = new CubeEditorHudPositioner(document, root);
  expect(root.hidden).toBe(true);
  const setProperty = jest.spyOn(root.style, 'setProperty');
  window.dispatchEvent(new Event('resize'));
  positioner.dispose();
  mountViewport();
  await Promise.resolve();
  await Promise.resolve();
  expect(root.hidden).toBe(true);
  expect(setProperty).not.toHaveBeenCalled();
  jest.restoreAllMocks();
});

/** Mount the native graph boundary with deterministic allocated screen bounds. */
function mountViewport(): void {
  const graph = document.createElement('div');
  graph.id = 'graph-canvas-container';
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  graph.append(canvas);
  jest.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 86,
    left: 0,
    top: 86,
    right: 800,
    bottom: 886,
    width: 800,
    height: 800,
    toJSON: () => ({}),
  });
  document.body.append(graph);
}
