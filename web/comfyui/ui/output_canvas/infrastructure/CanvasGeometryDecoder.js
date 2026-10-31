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
import { isCanvasSurfaceGeometry } from '../../core/embedded/CanvasRuntime.js';
import { isRecord } from '../../types/common.js';
import { authorizeCanvasMessage } from './CanvasMessageAuthority.js';
/**
 * Accept only authorized, finite, bounded geometry with explicit density and visibility.
 *
 * Gesture occlusions and the modal flag are retained only when present, so geometry without
 * them keeps its exact wire shape for hosts that do not route native input.
 */
export function decodeCanvasSurfaceGeometry(message, origin, authority) {
    if (!authorizeCanvasMessage(message, origin, authority) || !isRecord(message.geometry))
        return null;
    const raw = message.geometry;
    if (!isRecord(raw.origin) || !isRecord(raw.viewport) || !isRecord(raw.clip))
        return null;
    const value = {
        coordinateSpace: raw.coordinateSpace,
        units: raw.units,
        origin: { x: raw.origin.x, y: raw.origin.y },
        viewport: { width: raw.viewport.width, height: raw.viewport.height },
        clip: { x: raw.clip.x, y: raw.clip.y, width: raw.clip.width, height: raw.clip.height },
        physicalPixelsPerCssPixel: raw.physicalPixelsPerCssPixel,
        visible: raw.visible,
        focused: raw.focused,
        ...(Object.hasOwn(raw, 'occlusions')
            ? {
                occlusions: Array.isArray(raw.occlusions)
                    ? raw.occlusions.map((rect) => isRecord(rect)
                        ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
                        : rect)
                    : raw.occlusions,
            }
            : {}),
        ...(Object.hasOwn(raw, 'modal') ? { modal: raw.modal } : {}),
    };
    if (!isCanvasSurfaceGeometry(value))
        return null;
    const position = value.origin;
    const viewport = value.viewport;
    const clip = value.clip;
    return Object.freeze({
        coordinateSpace: 'client',
        units: 'css-pixels',
        origin: Object.freeze({ x: position.x, y: position.y }),
        viewport: Object.freeze({ width: viewport.width, height: viewport.height }),
        clip: Object.freeze({ x: clip.x, y: clip.y, width: clip.width, height: clip.height }),
        physicalPixelsPerCssPixel: value.physicalPixelsPerCssPixel,
        visible: value.visible,
        focused: value.focused,
        ...(value.occlusions
            ? { occlusions: Object.freeze(value.occlusions.map((rect) => Object.freeze({ ...rect }))) }
            : {}),
        ...(value.modal === undefined ? {} : { modal: value.modal }),
    });
}
