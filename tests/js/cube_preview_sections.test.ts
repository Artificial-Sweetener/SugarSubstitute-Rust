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
  layoutCubeCanvasPreviewSections,
  resolveCubeCanvasPreviewContentRect,
  resolveCubeCanvasPreviewSections,
  resolveCubePreviewItemGrid,
  resolveCubeOutputSections,
} from '../../frontend/comfyui/ui/surface/CubePreviewSections.js';

describe('CubePreviewSections', () => {
  test('retains every boundary output in graph order', () => {
    const snapshot = {
      outputs: [
        { id: 'image', label: 'image', items: [] },
        { id: 'mask', label: 'mask', items: [] },
      ],
    };

    expect(resolveCubeOutputSections(snapshot).map((output) => output.id)).toEqual([
      'image',
      'mask',
    ]);
  });

  test('retains every item produced by each boundary output', () => {
    const firstItem = { key: 'image-one', url: '/image-one.png', label: 'image one' };
    const secondItem = { key: 'image-two', url: '/image-two.png', label: 'image two' };

    expect(
      resolveCubeCanvasPreviewSections({
        outputs: [
          { id: 'image', label: 'image', items: [firstItem, secondItem] },
          { id: 'mask', label: 'mask', items: [] },
        ],
      }),
    ).toEqual([
      { canonicalName: 'image', items: [firstItem, secondItem] },
      { canonicalName: 'mask', items: [] },
    ]);
  });

  test('places two items side by side when that maximizes usable cell size', () => {
    expect(resolveCubePreviewItemGrid({ width: 450, height: 400 }, 2, 12)).toEqual({
      columns: 2,
      rows: 1,
    });
  });

  test('stacks two items when a narrow area makes rows more useful', () => {
    expect(resolveCubePreviewItemGrid({ width: 200, height: 400 }, 2, 12)).toEqual({
      columns: 1,
      rows: 2,
    });
  });

  test('lays out every canvas item while keeping output sections vertical', () => {
    const firstMask = { key: 'mask-one', url: '/mask-one.png', label: 'mask one' };
    const secondMask = { key: 'mask-two', url: '/mask-two.png', label: 'mask two' };
    const image = { key: 'image', url: '/image.png', label: 'image' };
    const sections = layoutCubeCanvasPreviewSections(
      { x: 0, y: 0, width: 500, height: 600 },
      {
        outputs: [
          { id: 'image', label: 'image', items: [image] },
          { id: 'mask', label: 'mask', items: [firstMask, secondMask] },
        ],
      },
    );

    expect(sections).toHaveLength(2);
    expect(sections[0]?.rect.y).toBeLessThan(sections[1]?.rect.y ?? 0);
    expect(sections[1]?.items.map(({ item }) => item)).toEqual([firstMask, secondMask]);
    expect(sections[1]?.items[0]?.rect.y).toBe(sections[1]?.items[1]?.rect.y);
    expect(sections[1]?.items[0]?.rect.x).toBeLessThan(sections[1]?.items[1]?.rect.x ?? 0);
  });

  test('divides the available height into equal full-width horizontal segments', () => {
    expect(
      dividePreviewIntoHorizontalSegments({ x: 10, y: 20, width: 100, height: 210 }, 2, 10),
    ).toEqual([
      { x: 10, y: 20, width: 100, height: 100 },
      { x: 10, y: 130, width: 100, height: 100 },
    ]);
  });

  test('reserves the authored Nodes 1 output-port corridor outside preview media', () => {
    const rail = { x: 100, y: 200, width: 320, height: 600 };

    const media = resolveCubeCanvasPreviewContentRect(rail);

    expect(media).toEqual({ x: 118, y: 206, width: 284, height: 588 });
    expect(rail.x + rail.width - (media.x + media.width)).toBe(18);
  });
});
