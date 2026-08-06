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
/** Verify fresh Cube sizing reserves bounded masonry and preview regions. */

import { describe, expect, test } from '@jest/globals';
import { resolveCubeInitialSurfaceSize } from '../../frontend/comfyui/ui/surface/CubeInitialSurfaceSize.js';

describe('resolveCubeInitialSurfaceSize', () => {
  test('reserves two default masonry columns, an input gutter, and output preview', () => {
    expect(resolveCubeInitialSurfaceSize({ surface: {}, hasInputs: true })).toEqual([920, 600]);
  });

  test('reserves the face preview independently from canonical boundary sockets', () => {
    expect(resolveCubeInitialSurfaceSize({ surface: {}, hasInputs: false })).toEqual([836, 600]);
    expect(resolveCubeInitialSurfaceSize({ surface: {}, hasInputs: true })).toEqual([920, 600]);
  });

  test('gives the installed Anima Prompt by Region face two columns and its preview rail', () => {
    expect(
      resolveCubeInitialSurfaceSize({
        surface: {
          minimum_column_width: 240,
          gap: 12,
          preview: { visible: true, width: 513.6844451311475 },
        },
        hasInputs: false,
      }),
    ).toEqual([1_030, 600]);
  });

  test('honors useful face settings without allowing persisted geometry to explode', () => {
    expect(
      resolveCubeInitialSurfaceSize({
        surface: {
          minimum_column_width: 10_000,
          gap: 10_000,
          preview: { visible: true, width: 10_000 },
        },
        hasInputs: true,
      }),
    ).toEqual([1_380, 600]);
  });

  test('does not reserve a preview when the Cube face intentionally hides it', () => {
    expect(
      resolveCubeInitialSurfaceSize({
        surface: { preview: { visible: false, width: 500 } },
        hasInputs: true,
      }),
    ).toEqual([592, 600]);
  });
});
