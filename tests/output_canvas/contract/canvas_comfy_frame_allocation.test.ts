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

/** Build only the installed public frame/chrome shape, leaving all renderer children in place. */
function frame() {
  const root = document.createElement('div');
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
  return { root, chrome, panel, canvas, nodes };
}

test('allocates a common graph origin and compensates only identified native chrome', () => {
  const f = frame();
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  adapter.apply(480);
  expect(f.root.style.marginLeft).toBe('480px');
  expect(f.root.style.width).toBe('calc(100% - 480px)');
  expect(f.chrome.style.left).toBe('-480px');
  expect(f.chrome.style.width).toBe('calc(100% + 480px)');
  expect(f.root.style.clipPath).toBe('inset(0px 0px 0px -480px)');
  expect(f.root.children).toHaveLength(3);
  expect(f.canvas.parentElement).toBe(f.root);
  expect(f.nodes.parentElement).toBe(f.root);
  expect(f.canvas.style.width).toBe('');
  adapter.apply(0);
  expect(f.root.style.cssText).toBe('');
  expect(f.chrome.style.cssText).toBe('');
  adapter.dispose();
});

test('restores pre-existing inline styles and priorities on disposal and rejects unsupported roots', () => {
  const f = frame();
  f.root.style.setProperty('width', '90%', 'important');
  f.root.style.setProperty('overflow', 'clip');
  f.chrome.style.setProperty('left', '2px');
  const original = f.root.style.cssText;
  const chromeOriginal = f.chrome.style.cssText;
  const adapter = new ComfyCanvasFrameAllocation(f.root, f.panel);
  expect(() => adapter.apply(-1)).toThrow(RangeError);
  adapter.apply(400);
  adapter.dispose();
  expect(f.root.style.cssText).toBe(original);
  expect(f.chrome.style.cssText).toBe(chromeOriginal);
  adapter.apply(500);
  expect(f.root.style.cssText).toBe(original);
  expect(() => new ComfyCanvasFrameAllocation(document.createElement('div'), f.panel)).toThrow(
    'unsupported',
  );
  f.canvas.remove();
  expect(() => new ComfyCanvasFrameAllocation(f.root, f.panel)).toThrow('unsupported');
});
