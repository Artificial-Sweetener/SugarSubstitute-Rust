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
/** Describe browser camera geometry in CSS pixels with explicit physical display density. */
/** Refuse hidden or invalid geometry before it can corrupt saved inspection state. */
export function canvasCameraGeometry(image, viewport, density) {
    if (!image ||
        ![image.width, image.height, viewport.width, viewport.height, density].every((value) => Number.isFinite(value) && value > 0))
        return null;
    const fit = Math.min(viewport.width / image.width, viewport.height / image.height);
    if (!Number.isSafeInteger(image.width) ||
        !Number.isSafeInteger(image.height) ||
        ![
            fit,
            image.width * fit,
            image.height * fit,
            10 / density,
            (image.width * 10) / density,
            (image.height * 10) / density,
        ].every((value) => Number.isFinite(value) && value > 0))
        return null;
    return Object.freeze({
        image: Object.freeze({ ...image }),
        viewport: Object.freeze({ ...viewport }),
        density,
    });
}
/** Compare display scales without making exact snap modes depend on rounding noise. */
export function sameCanvasScale(left, right) {
    return Math.abs(left - right) <= Math.max(left, right, Number.EPSILON) * 1e-8;
}
/** Fill the available viewport while preserving aspect ratio, including enlargement of previews. */
export function canvasFitScale(geometry) {
    return Math.min(geometry.viewport.width / geometry.image.width, geometry.viewport.height / geometry.image.height);
}
/** Report a truthful mode for this image's decoded pixels. */
export function canvasCameraMode(scale, geometry) {
    return sameCanvasScale(scale, canvasFitScale(geometry))
        ? 'fit'
        : sameCanvasScale(scale, 1 / geometry.density)
            ? 'native'
            : 'custom';
}
/** Center an explicit scale, with Fit/native limits enforced by the camera authority. */
export function centeredCanvasCamera(geometry, mode = 'fit') {
    const scale = mode === 'fit' ? canvasFitScale(geometry) : 1 / geometry.density;
    return Object.freeze({
        scale,
        x: (geometry.viewport.width - geometry.image.width * scale) / 2,
        y: (geometry.viewport.height - geometry.image.height * scale) / 2,
        mode: canvasCameraMode(scale, geometry),
    });
}
/** Clamp an overflowing axis to website-style pan bounds and center a fitting axis. */
function constrainedAxis(translation, imageExtent, viewportExtent) {
    if (imageExtent <= viewportExtent)
        return (viewportExtent - imageExtent) / 2;
    const margin = viewportExtent * 0.2;
    return Math.max(viewportExtent - imageExtent - margin, Math.min(margin, translation));
}
/** Settle transient anchored slack through one shared scale and pan policy. */
export function settleCanvasCamera(camera, geometry) {
    const scale = constrainedScale(camera.scale, geometry);
    return Object.freeze({
        scale,
        x: constrainedAxis(camera.x, geometry.image.width * scale, geometry.viewport.width),
        y: constrainedAxis(camera.y, geometry.image.height * scale, geometry.viewport.height),
        mode: canvasCameraMode(scale, geometry),
    });
}
/** Keep the same decoded image point beneath an anchor before settlement constraints. */
export function anchoredCanvasCamera(camera, scale, anchor, geometry) {
    if (![scale, anchor.x, anchor.y].every(Number.isFinite) || scale <= 0)
        throw new RangeError('Canvas anchored scale or position is invalid.');
    const next = constrainedScale(scale, geometry);
    return Object.freeze({
        scale: next,
        x: anchor.x - ((anchor.x - camera.x) * next) / camera.scale,
        y: anchor.y - ((anchor.y - camera.y) * next) / camera.scale,
        mode: canvasCameraMode(next, geometry),
    });
}
/** Land exactly on crossed native/Fit targets and allow the next input to depart. */
export function zoomCanvasCamera(camera, factor, anchor, geometry) {
    if (!Number.isFinite(factor) || factor <= 0)
        throw new RangeError('Canvas zoom factor is invalid.');
    let scale = camera.scale * factor;
    for (const target of [1 / geometry.density, canvasFitScale(geometry)]) {
        if (!sameCanvasScale(camera.scale, target) &&
            ((camera.scale < target && scale >= target) || (camera.scale > target && scale <= target))) {
            scale = target;
            break;
        }
    }
    return anchoredCanvasCamera(camera, scale, anchor, geometry);
}
/** Pan in CSS coordinates through the same constraints used by all navigation inputs. */
export function panCanvasCamera(camera, delta, geometry) {
    if (![delta.x, delta.y].every(Number.isFinite))
        throw new RangeError('Canvas pan delta is invalid.');
    return settleCanvasCamera({ ...camera, x: camera.x + delta.x, y: camera.y + delta.y }, geometry);
}
/** Toggle Fit and pointer-anchored native size without oscillating when both coincide. */
export function doubleClickCanvasCamera(camera, anchor, geometry) {
    if (camera.mode !== 'fit' || sameCanvasScale(canvasFitScale(geometry), 1 / geometry.density))
        return centeredCanvasCamera(geometry);
    return settleCanvasCamera(anchoredCanvasCamera(camera, 1 / geometry.density, anchor, geometry), geometry);
}
/** Normalize browser wheel units while preserving fractional trackpad movement. */
export function canvasWheelFactor(delta, units, viewportHeight) {
    if (!Number.isFinite(delta) || !Number.isFinite(viewportHeight) || viewportHeight <= 0)
        return 1;
    const pixels = delta * (units === 1 ? 16 : units === 2 ? viewportHeight : 1);
    return Math.pow(1.25, -Math.max(-1200, Math.min(1200, pixels)) / 100);
}
/** Keep Fit and native reachable when a small image's Fit exceeds physical 1:1. */
function constrainedScale(scale, geometry) {
    const fit = canvasFitScale(geometry);
    const minimum = Math.min(fit, 1 / geometry.density);
    const maximum = Math.max(fit, 10 / geometry.density);
    return Math.max(minimum, Math.min(maximum, scale));
}
