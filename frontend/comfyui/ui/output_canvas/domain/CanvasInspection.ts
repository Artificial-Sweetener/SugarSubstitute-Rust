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
/** Capture and project image-relative scale per physical pixel, matching QPane inspection semantics. */

import type { NormalizedInspection } from './CanvasViewState.js';
import {
  canvasCameraMode,
  centeredCanvasCamera,
  settleCanvasCamera,
  type CanvasCamera,
  type CanvasCameraGeometry,
} from './CanvasCamera.js';

export interface CanvasInspection {
  readonly region: NormalizedInspection;
  readonly mode: CanvasCamera['mode'];
}

/** Exclude viewport extent from magnification so resize reveals more content at the same scale. */
export function captureCanvasInspection(
  camera: CanvasCamera,
  geometry: CanvasCameraGeometry,
): CanvasInspection {
  const physicalScale = camera.scale * geometry.density;
  return Object.freeze({
    mode: camera.mode,
    region: Object.freeze({
      centerX: (geometry.viewport.width / 2 - camera.x) / (geometry.image.width * camera.scale),
      centerY: (geometry.viewport.height / 2 - camera.y) / (geometry.image.height * camera.scale),
      spanX: 1 / (geometry.image.width * physicalScale),
      spanY: 1 / (geometry.image.height * physicalScale),
    }),
  });
}

/** Contain relative scale across differing aspect ratios and report the receiving image's own mode. */
export function projectCanvasInspection(
  inspection: CanvasInspection,
  geometry: CanvasCameraGeometry,
): CanvasCamera {
  if (inspection.mode === 'fit') return centeredCanvasCamera(geometry);
  const region = inspection.region;
  if (
    ![region.centerX, region.centerY, region.spanX, region.spanY].every(Number.isFinite) ||
    region.spanX <= 0 ||
    region.spanY <= 0
  )
    return centeredCanvasCamera(geometry);
  const scale =
    Math.min(
      1 / (geometry.image.width * region.spanX),
      1 / (geometry.image.height * region.spanY),
    ) / geometry.density;
  return settleCanvasCamera(
    {
      scale,
      x: geometry.viewport.width / 2 - region.centerX * geometry.image.width * scale,
      y: geometry.viewport.height / 2 - region.centerY * geometry.image.height * scale,
      mode: canvasCameraMode(scale, geometry),
    },
    geometry,
  );
}

/** Resize or change density without scaling custom content with the viewport. */
export function resizeCanvasCamera(
  camera: CanvasCamera,
  previous: CanvasCameraGeometry,
  next: CanvasCameraGeometry,
): CanvasCamera {
  if (camera.mode === 'fit') return centeredCanvasCamera(next);
  const inspection = captureCanvasInspection(camera, previous);
  if (camera.mode === 'native') {
    const scale = 1 / next.density;
    return settleCanvasCamera(
      {
        ...camera,
        scale,
        x: next.viewport.width / 2 - inspection.region.centerX * next.image.width * scale,
        y: next.viewport.height / 2 - inspection.region.centerY * next.image.height * scale,
      },
      next,
    );
  }
  return projectCanvasInspection(inspection, next);
}
