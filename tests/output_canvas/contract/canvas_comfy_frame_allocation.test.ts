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
/** Prove graph-frame qualification and exact reversible ownership without replacing Comfy's renderers. */

import { ComfyCanvasFrameAllocation } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasFrameAllocation.js';
import { jest } from '@jest/globals';

/** Build only the installed public frame/chrome shape, leaving all renderer children in place. */
function frame() {
  const parent = document.createElement('div');
  parent.className = 'comfyui-body';
  const root = document.createElement('div');
  parent.append(root);
  root.id = 'graph-canvas-container';
  const chrome = document.createElement('div');
  const panel = document.createElement('div');
  panel.className = 'graph-canvas-panel';
  chrome.append(panel);
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  const nodes = document.createElement('div');
  nodes.className = 'ph-no-capture';
  root.append(chrome, canvas, nodes);
  return { parent, root, chrome, panel, canvas, nodes };
}

test('allocates a common graph origin and compensates only identified native chrome', () => {
  const f = frame();
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.marginLeft).toBe('0px');
  expect(f.root.style.width).toBe('calc(100% - 480px)');
  expect(f.panel.style.getPropertyValue('--sc-output-workspace-reserved-width')).toBe('480px');
  expect(f.chrome.style.left).toBe('0px');
  expect(f.chrome.style.width).toBe('calc(100% + 480px)');
  expect(f.root.style.clipPath).toBe('inset(0px -480px 0px 0px)');
  expect(f.parent.style.overflow).toBe('clip');
  expect(f.root.children).toHaveLength(3);
  expect(f.canvas.parentElement).toBe(f.root);
  expect(f.nodes.parentElement).toBe(f.root);
  expect(f.canvas.style.width).toBe('');
  adapter.apply(0);
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
  expect(f.parent.style.cssText).toBe('');
  expect(f.panel.style.cssText).toBe('');
  adapter.dispose();
});

test('restores pre-existing inline styles and priorities on disposal and rejects unsupported roots', () => {
  const f = frame();
  f.root.style.setProperty('width', '90%', 'important');
  f.root.style.setProperty('overflow', 'clip');
  f.parent.style.setProperty('overflow', 'hidden', 'important');
  f.chrome.style.setProperty('left', '2px');
  f.panel.style.setProperty('--sc-output-workspace-reserved-width', '10px', 'important');
  const original = f.root.style.cssText;
  const chromeOriginal = f.chrome.style.cssText;
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  expect(() => adapter.apply(-1)).toThrow(RangeError);
  adapter.apply(400);
  adapter.dispose();
  expect(f.root.style.cssText).toBe(original);
  expect(f.chrome.style.cssText).toBe(chromeOriginal);
  expect(f.parent.style.getPropertyValue('overflow')).toBe('hidden');
  expect(f.parent.style.getPropertyPriority('overflow')).toBe('important');
  expect(f.panel.style.getPropertyValue('--sc-output-workspace-reserved-width')).toBe('10px');
  expect(f.panel.style.getPropertyPriority('--sc-output-workspace-reserved-width')).toBe(
    'important',
  );
  adapter.apply(500);
  expect(f.root.style.cssText).toBe(original);
  expect(() => new ComfyCanvasFrameAllocation(document.createElement('div'), f.panel)).toThrow(
    'unsupported',
  );
  f.canvas.remove();
  expect(() => new ComfyCanvasFrameAllocation(f.root, f.panel)).toThrow('unsupported');
});

test('allocates the real graph below chrome even with the output canvas closed and restores on suspension', () => {
  const f = frame();
  jest.spyOn(f.parent, 'getBoundingClientRect').mockReturnValue(rectangle(0, 0, 1200, 900));
  let top = 86;
  jest
    .spyOn(f.panel, 'getBoundingClientRect')
    .mockImplementation(() => rectangle(4, top, 1192, 896 - top));
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.marginTop).toBe('86px');
  expect(f.root.style.height).toBe('calc(100% - 90px)');
  expect(f.chrome.style.top).toBe('-86px');
  expect(f.chrome.style.height).toBe('calc(100% + 90px)');
  expect(f.root.style.marginLeft).toBe('4px');
  expect(f.root.style.width).toBe('calc(100% - 488px)');
  expect(f.chrome.style.left).toBe('-4px');
  expect(f.root.style.clipPath).toBe('inset(-86px -484px -4px -4px)');
  adapter.apply(0);
  expect(f.root.style.width).toBe('calc(100% - 8px)');
  expect(f.root.style.marginTop).toBe('86px');
  top = 120;
  adapter.apply(0);
  expect(f.root.style.marginTop).toBe('120px');
  expect(f.root.style.height).toBe('calc(100% - 124px)');
  const mutations = new MutationObserver(() => undefined);
  mutations.observe(f.root, { attributes: true, subtree: true });
  adapter.apply(0);
  expect(mutations.takeRecords()).toHaveLength(0);
  mutations.disconnect();
  adapter.suspend();
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
  adapter.apply(0);
  expect(f.root.style.marginTop).toBe('120px');
  adapter.dispose();
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
});

/** Supply stable browser geometry without relying on JSDOM layout. */
function rectangle(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    toJSON: () => ({ x, y, width, height }),
  };
}
