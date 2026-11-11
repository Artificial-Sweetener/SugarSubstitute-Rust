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

import type { CanvasSelection, NormalizedInspection } from './CanvasViewState.js';
import {
  canvasCameraMode,
  centeredCanvasCamera,
  type CanvasCamera,
  type CanvasCameraGeometry,
} from './CanvasCamera.js';

/** Scope corresponding source/batch cameras to the actual accepted scene run and scene. */
export interface CanvasInspectionSlot extends CanvasSelection {
  readonly scene?: readonly [run: string, key: string];
}

/** Compare camera addresses without treating another scene's same source/batch as the same image. */
export function canvasInspectionSlotKey(slot: CanvasInspectionSlot): string {
  return JSON.stringify([slot.scene ?? null, slot.sourceId, slot.batchIndex]);
}

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
    throw new RangeError('Saved canvas inspection is invalid.');
  const scale =
    Math.min(1 / region.spanX / geometry.image.width, 1 / region.spanY / geometry.image.height) /
    geometry.density;
  return cameraAtInspectionCenter(inspection, geometry, scale);
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
    return cameraAtInspectionCenter(inspection, next, scale);
  }
  return projectCanvasInspection(inspection, next);
}

/** Restore finite numerical geometry without applying interactive wheel or pan limits to saved state. */
function cameraAtInspectionCenter(
  inspection: CanvasInspection,
  geometry: CanvasCameraGeometry,
  scale: number,
): CanvasCamera {
  const width = geometry.image.width * scale;
  const height = geometry.image.height * scale;
  const x = geometry.viewport.width / 2 - inspection.region.centerX * width;
  const y = geometry.viewport.height / 2 - inspection.region.centerY * height;
  if (
    ![scale, width, height].every((value) => Number.isFinite(value) && value > 0) ||
    ![x, y].every(Number.isFinite)
  )
    throw new RangeError('Saved canvas inspection cannot be represented by this image geometry.');
  return Object.freeze({ scale, x, y, mode: canvasCameraMode(scale, geometry) });
}
