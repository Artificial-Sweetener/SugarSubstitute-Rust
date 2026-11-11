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
/** Prove title-bar placement survives native renderer geometry and graph zoom. */
import { placeNativeOverridePin } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverridePinPlacement.js';
import type { ComfyApplication, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Model the external renderer geometry without requiring the browser DOMRect constructor. */
function rect(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    width,
    height,
    top: y,
    left: x,
    right: x + width,
    bottom: y + height,
    toJSON: () => ({}),
  };
}

/** Supply the settled dimensions exposed by the external DOM widget. */
function fixture() {
  const root = document.createElement('div');
  root.dataset.scOverridesGraph = '';
  const pin = document.createElement('button');
  pin.className = 'sc-overrides-menu';
  root.append(pin);
  Object.defineProperty(root, 'offsetWidth', { value: 380 });
  Object.defineProperty(pin, 'offsetHeight', { value: 14 });
  return { root, pin };
}

test.each([0.5, 1, 2])('aligns a Nodes 1 pin with the native title at zoom %s', (scale) => {
  const { root } = fixture();
  const canvas = document.createElement('canvas');
  canvas.getBoundingClientRect = () => rect(40, 60, 1000, 800);
  root.getBoundingClientRect = () => rect(40 + 120 * scale, 60 + 230 * scale, 380 * scale, 100);
  const node: ComfyNode = { pos: [100, 200], size: [400, 100] };
  const app: ComfyApplication = { canvas: { canvas, ds: { scale, offset: [10, 20] } } };
  placeNativeOverridePin(root, node, app, { NODE_TITLE_HEIGHT: 30 });
  expect(root.style.getPropertyValue('--sc-overrides-pin-top')).toBe('-32px');
  expect(root.style.getPropertyValue('--sc-overrides-pin-right')).toBe('0px');
});

test('aligns a Nodes 2 pin to the actual header rather than the first field row', () => {
  const { root } = fixture();
  const card = document.createElement('div');
  card.className = 'lg-node';
  const header = document.createElement('div');
  header.className = 'lg-node-header';
  card.append(header, root);
  header.getBoundingClientRect = () => rect(0, 100, 400, 24);
  root.getBoundingClientRect = () => rect(10, 140, 380, 100);
  placeNativeOverridePin(root, {}, null, {});
  expect(root.style.getPropertyValue('--sc-overrides-pin-top')).toBe('-35px');
  expect(root.style.getPropertyValue('--sc-overrides-pin-right')).toBe('0px');
});

test('leaves unmounted widgets and masonry-owned pin placement alone', () => {
  const { root } = fixture();
  placeNativeOverridePin(root, {}, null, {});
  expect(root.style.cssText).toBe('');
  root.removeAttribute('data-sc-overrides-graph');
  root.getBoundingClientRect = () => rect(0, 0, 380, 100);
  placeNativeOverridePin(root, {}, null, {});
  expect(root.style.cssText).toBe('');
});
