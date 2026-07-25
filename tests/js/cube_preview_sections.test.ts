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
/** Verify renderer-neutral Cube preview section policy. */

import {
  dividePreviewIntoHorizontalSegments,
  resolveCubeCanvasPreviewSections,
  resolveCubeOutputSections,
} from '../../frontend/comfyui/ui/surface/CubePreviewSections.js';

describe('CubePreviewSections', () => {
  test('retains every boundary output in graph order', () => {
    const snapshot = {
      outputs: [
        { id: 'image', label: 'image', items: [] },
        { id: 'mask', label: 'mask', items: [] },
      ],
      internalItems: [],
    };

    expect(resolveCubeOutputSections(snapshot).map((output) => output.id)).toEqual([
      'image',
      'mask',
    ]);
  });

  test('uses internal previews only to fill empty canvas output sections', () => {
    const outputItem = { key: 'image', url: '/image.png', label: 'image' };
    const internalItem = { key: 'mask', url: '/mask.png', label: 'mask' };

    expect(
      resolveCubeCanvasPreviewSections({
        outputs: [
          { id: 'image', label: 'image', items: [outputItem] },
          { id: 'mask', label: 'mask', items: [] },
        ],
        internalItems: [internalItem],
      }),
    ).toEqual([
      { canonicalName: 'image', item: outputItem },
      { canonicalName: 'mask', item: internalItem },
    ]);
  });

  test('divides the available height into equal full-width horizontal segments', () => {
    expect(
      dividePreviewIntoHorizontalSegments({ x: 10, y: 20, width: 100, height: 210 }, 2, 10),
    ).toEqual([
      { x: 10, y: 20, width: 100, height: 100 },
      { x: 10, y: 130, width: 100, height: 100 },
    ]);
  });
});
