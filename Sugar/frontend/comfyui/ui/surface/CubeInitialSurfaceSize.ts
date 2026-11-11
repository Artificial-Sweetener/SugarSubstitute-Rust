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
/** Size fresh Cube faces around their visible masonry and preview responsibilities. */

import type {
  CubeInitialSizeRequest,
  CubeInitialSizeResolver,
} from '../cube/geometry/CubeInitialSizePolicy.js';
import { CUBE_INPUT_GUTTER_WIDTH } from './CubePortGutterLayout.js';
import {
  CUBE_DOM_SURFACE_SECTION_GAP,
  CUBE_SURFACE_FRAME_PADDING,
  CUBE_SURFACE_SECTION_GAP,
  CUBE_VUE_CONTENT_HORIZONTAL_INSET,
} from './CubeSurfaceGeometry.js';
import { createDefaultCubeSurfaceState, parseCubeSurfaceState } from './CubeSurfaceState.js';

const INITIAL_MASONRY_COLUMN_COUNT = 2;
const INITIAL_CUBE_HEIGHT = 600;
const MAXIMUM_INITIAL_COLUMN_WIDTH = 360;
const MAXIMUM_INITIAL_MASONRY_GAP = 32;
const MINIMUM_INITIAL_PREVIEW_WIDTH = 240;
const MAXIMUM_INITIAL_PREVIEW_WIDTH = 520;

/** Resolve a bounded fresh frame with two masonry columns and a useful output preview. */
export const resolveCubeInitialSurfaceSize: CubeInitialSizeResolver = (
  request: CubeInitialSizeRequest,
) => {
  const defaults = createDefaultCubeSurfaceState();
  const state = parseCubeSurfaceState(request.surface);
  const columnWidth = clamp(
    state.minimumColumnWidth,
    defaults.minimumColumnWidth,
    MAXIMUM_INITIAL_COLUMN_WIDTH,
  );
  const masonryGap = clamp(state.gap, 0, MAXIMUM_INITIAL_MASONRY_GAP);
  const masonryWidth =
    INITIAL_MASONRY_COLUMN_COUNT * columnWidth + (INITIAL_MASONRY_COLUMN_COUNT - 1) * masonryGap;
  const previewWidth = state.preview.visible
    ? clamp(state.preview.width, MINIMUM_INITIAL_PREVIEW_WIDTH, MAXIMUM_INITIAL_PREVIEW_WIDTH)
    : 0;
  const canvasLeftInset = request.hasInputs ? CUBE_INPUT_GUTTER_WIDTH : CUBE_SURFACE_FRAME_PADDING;
  const canvasWidth =
    canvasLeftInset +
    masonryWidth +
    (previewWidth > 0 ? CUBE_SURFACE_SECTION_GAP + previewWidth : CUBE_SURFACE_FRAME_PADDING);
  const vueWidth =
    CUBE_VUE_CONTENT_HORIZONTAL_INSET +
    (request.hasInputs ? CUBE_INPUT_GUTTER_WIDTH : 0) +
    masonryWidth +
    (previewWidth > 0 ? CUBE_DOM_SURFACE_SECTION_GAP + previewWidth : 0);

  return [Math.ceil(Math.max(canvasWidth, vueWidth)), INITIAL_CUBE_HEIGHT];
};

/** Keep persisted presentation dimensions within predictable fresh-placement bounds. */
function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, Number.isFinite(value) ? value : minimum));
}
