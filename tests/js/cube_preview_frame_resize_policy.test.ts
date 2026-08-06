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
/** Verify that frame resizing and divider allocation remain separate commands. */

import { CubePreviewFrameResizePolicy } from '../../frontend/comfyui/ui/surface/CubePreviewFrameResizePolicy.js';

const range = { minimum: 160, maximum: 800 };

describe('CubePreviewFrameResizePolicy', () => {
  test('allocates outer frame width changes to the preview', () => {
    const policy = new CubePreviewFrameResizePolicy();

    expect(policy.resolve({ frameWidth: 900, previewWidth: 320, range, active: true })).toBe(320);
    expect(policy.resolve({ frameWidth: 1100, previewWidth: 320, range, active: true })).toBe(520);
    expect(policy.resolve({ frameWidth: 1000, previewWidth: 520, range, active: true })).toBe(420);
  });

  test('uses divider changes as the new masonry allocation baseline', () => {
    const policy = new CubePreviewFrameResizePolicy();

    policy.resolve({ frameWidth: 900, previewWidth: 320, range, active: true });
    expect(policy.resolve({ frameWidth: 900, previewWidth: 400, range, active: true })).toBe(400);
    expect(policy.resolve({ frameWidth: 1200, previewWidth: 400, range, active: true })).toBe(700);
  });

  test('does not mutate or carry a resize baseline through stacked preview mode', () => {
    const policy = new CubePreviewFrameResizePolicy();

    policy.resolve({ frameWidth: 900, previewWidth: 320, range, active: true });
    expect(policy.resolve({ frameWidth: 600, previewWidth: 92, range, active: false })).toBe(92);
    expect(policy.resolve({ frameWidth: 700, previewWidth: 320, range, active: true })).toBe(320);
  });

  test('clamps preview growth to the current renderer allocation', () => {
    const policy = new CubePreviewFrameResizePolicy();

    policy.resolve({ frameWidth: 900, previewWidth: 700, range, active: true });
    expect(policy.resolve({ frameWidth: 1200, previewWidth: 700, range, active: true })).toBe(800);
  });
});
