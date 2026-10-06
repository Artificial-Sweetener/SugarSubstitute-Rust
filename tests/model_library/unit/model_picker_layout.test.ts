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
/** Prove the image wall matches rows rendered by Substitute's headless layout owner. */
import {
  layoutModelWall,
  visibleModelRows,
} from '../../../frontend/comfyui/ui/model_library/ModelPickerLayout.js';

describe('Substitute model wall layout', () => {
  test.each([
    {
      width: 560,
      aspects: new Array<number>(12).fill(0.72),
      heights: [190.27777777777777, 190.27777777777777, 190.27777777777777],
      counts: [4, 4, 4],
    },
    {
      width: 420,
      aspects: [0.6, 1.5, 0.72, 1, 2, 0.65, 0.75, 0.8],
      heights: [198.09523809523807, 241.86046511627907, 210, 187.27272727272725],
      counts: [2, 2, 1, 3],
    },
    { width: 260, aspects: [0.2, 6, 0.72, 1], heights: [156, 148.8372093023256], counts: [2, 2] },
    {
      width: 700,
      aspects: new Array<number>(5).fill(0.72),
      heights: [190.00000000000003],
      counts: [5],
    },
  ])('matches Substitute at $width px', ({ width, aspects, heights, counts }) => {
    const rows = layoutModelWall(aspects, width);
    expect(rows.map((row) => row.tiles.length)).toEqual(counts);
    rows.forEach((row, index) => expect(row.height).toBeCloseTo(heights[index] ?? 0, 8));
    expect(rows.flatMap((row) => row.tiles.map((tile) => tile.index))).toEqual(
      aspects.map((_, index) => index),
    );
    for (const row of rows) {
      const last = row.tiles.at(-1);
      expect((last?.left ?? 0) + (last?.width ?? 0)).toBeLessThanOrEqual(width + 0.001);
    }
  });

  test('mounts only nearby rows in a large catalog while preserving total geometry', () => {
    const rows = layoutModelWall(new Array<number>(10000).fill(0.72), 560);
    const visible = visibleModelRows(rows, 100000, 630);
    expect(rows).toHaveLength(2500);
    expect(visible.length).toBeLessThan(12);
    expect(visible[0]?.top).toBeGreaterThan(99000);
    expect(rows.at(-1)?.top).toBeGreaterThan(400000);
  });
});
