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
/** Normalize surface geometry without mixing CSS and physical coordinate spaces. */

import { isRecord } from '../../types/common.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import { authorizeCanvasMessage, type CanvasMessageAuthority } from './CanvasMessageAuthority.js';

/** Accept only authorized, finite, bounded geometry with explicit density and visibility. */
export function decodeCanvasSurfaceGeometry(
  message: unknown,
  origin: unknown,
  authority: CanvasMessageAuthority,
): CanvasSurfaceGeometry | null {
  if (!authorizeCanvasMessage(message, origin, authority) || !isRecord(message.geometry))
    return null;
  const value = message.geometry;
  if (
    value.coordinateSpace !== 'client' ||
    value.units !== 'css-pixels' ||
    !isRecord(value.origin) ||
    !isRecord(value.viewport) ||
    !isRecord(value.clip)
  )
    return null;
  const position = value.origin;
  const viewport = value.viewport;
  const clip = value.clip;
  if (
    !coordinate(position.x) ||
    !coordinate(position.y) ||
    !extent(viewport.width) ||
    !extent(viewport.height) ||
    !coordinate(clip.x) ||
    !coordinate(clip.y) ||
    !extent(clip.width) ||
    !extent(clip.height) ||
    typeof value.physicalPixelsPerCssPixel !== 'number' ||
    !Number.isFinite(value.physicalPixelsPerCssPixel) ||
    value.physicalPixelsPerCssPixel <= 0 ||
    value.physicalPixelsPerCssPixel > 64 ||
    typeof value.visible !== 'boolean' ||
    typeof value.focused !== 'boolean'
  )
    return null;
  return Object.freeze({
    coordinateSpace: 'client',
    units: 'css-pixels',
    origin: Object.freeze({ x: position.x, y: position.y }),
    viewport: Object.freeze({ width: viewport.width, height: viewport.height }),
    clip: Object.freeze({ x: clip.x, y: clip.y, width: clip.width, height: clip.height }),
    physicalPixelsPerCssPixel: value.physicalPixelsPerCssPixel,
    visible: value.visible,
    focused: value.focused,
  });
}

/** Bound screen coordinates while allowing surfaces on monitors left of the primary display. */
function coordinate(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1_000_000;
}

/** Allow hidden zero-sized surfaces without accepting negative or unbounded allocations. */
function extent(value: unknown): value is number {
  return coordinate(value) && value >= 0;
}
