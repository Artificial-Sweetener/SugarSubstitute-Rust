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
/** Preserve selected owned-control appearance across native renderer suspension and palette changes. */
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { OwnedSurfaceAppearance } from '../../../frontend/comfyui/ui/surface/appearance/OwnedSurfaceAppearance.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';

test('retain Fluent for owned editors when native cards expose the Comfy renderer palette', () => {
  const native = new AppearanceState();
  publishVisualStyle(document, 'fluent');
  const owned = new OwnedSurfaceAppearance(document, native);
  native.update('comfy', true);
  expect(owned.appearance).toEqual({ style: 'fluent', dark: true });
  native.update('comfy', false);
  expect(owned.appearance).toEqual({ style: 'fluent', dark: false });
  native.dispose();
  document.documentElement.removeAttribute('data-sc-visual-style');
});

test('follow selected style and native palette and release both event subscriptions', () => {
  const native = new AppearanceState();
  const owned = new OwnedSurfaceAppearance(document, native);
  const snapshots: unknown[] = [];
  const release = owned.subscribeAppearance(() => snapshots.push(owned.appearance));
  publishVisualStyle(document, 'spectrum');
  native.update('comfy', false);
  expect(snapshots).toEqual([
    { style: 'spectrum', dark: true },
    { style: 'spectrum', dark: false },
  ]);
  release();
  publishVisualStyle(document, 'fluent');
  native.update('comfy', true);
  expect(snapshots).toHaveLength(2);
  native.dispose();
  document.documentElement.removeAttribute('data-sc-visual-style');
});
