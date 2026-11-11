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
/** Prove exact projected media replacement preserves native graph image state and unrelated cards. */
import {
  NativeNodeMediaReplacement,
  isNativeNodeMediaPresented,
  withNativeNodeMediaPresentation,
} from '../../../frontend/comfyui/ui/surface/NativeNodeMediaPresentation.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

test('suppresses only the leased card and restores exact native media properties after drawing or failure', () => {
  const root = document.createElement('article'),
    target = document.createElement('div'),
    other = document.createElement('div');
  root.append(target);
  document.body.append(root, other);
  const images = [{ source: 'native.png' }],
    node: ComfyNode = { id: 1, imgs: images };
  const replacement = new NativeNodeMediaReplacement(root);
  expect(isNativeNodeMediaPresented(target)).toBe(false);
  expect(isNativeNodeMediaPresented(other)).toBe(true);
  const inside = withNativeNodeMediaPresentation(target, node, () => ({
    images: node.imgs,
    animated: node.animatedImages,
  }));
  expect(inside).toEqual({ images: [], animated: [] });
  expect(node.imgs).toBe(images);
  expect(Object.hasOwn(node, 'animatedImages')).toBe(false);
  expect(() =>
    withNativeNodeMediaPresentation(target, node, () => {
      throw new Error('Native drawing failed.');
    }),
  ).toThrow('drawing failed');
  expect(node.imgs).toBe(images);
  expect(Object.hasOwn(node, 'animatedImages')).toBe(false);
  replacement.dispose();
  expect(isNativeNodeMediaPresented(target)).toBe(true);
  root.remove();
  other.remove();
});
