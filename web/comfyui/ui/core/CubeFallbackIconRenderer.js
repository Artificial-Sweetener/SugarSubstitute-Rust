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
/**
 * Render SugarSubstitute-compatible fallback cube-definition icons.
 */
export const FALLBACK_ICON_STYLE = Object.freeze({
    fontFamily: 'Segoe UI',
    fontWeight: 700,
    inset: 2,
    renderSize: 96,
});
const FALLBACK_ICON_FONT_SCALE = 62 / FALLBACK_ICON_STYLE.renderSize;
function fallbackStyle(model) {
    const source = model?.fallback ?? {};
    const renderSize = Number(source.renderSize);
    return {
        fontFamily: typeof source.fontFamily === 'string' && source.fontFamily.trim()
            ? source.fontFamily.trim()
            : FALLBACK_ICON_STYLE.fontFamily,
        fontWeight: typeof source.fontWeight === 'number' && Number.isFinite(source.fontWeight)
            ? source.fontWeight
            : FALLBACK_ICON_STYLE.fontWeight,
        inset: typeof source.inset === 'number' && Number.isFinite(source.inset)
            ? source.inset
            : FALLBACK_ICON_STYLE.inset,
        renderSize: Number.isFinite(renderSize) && renderSize > 0 ? renderSize : FALLBACK_ICON_STYLE.renderSize,
    };
}
/**
 * Return the shared fallback icon model used by DOM and canvas surfaces.
 */
export function createFallbackIconModel(initials) {
    return {
        kind: 'initials',
        initials: initials || '?',
        fallback: { ...FALLBACK_ICON_STYLE },
    };
}
/**
 * Return the CSS canvas font string for one fallback icon size.
 */
export function resolveFallbackIconFont(model, pixelSize) {
    const style = fallbackStyle(model);
    const size = Math.max(1, Math.round(pixelSize));
    return `${style.fontWeight} ${size}px "${style.fontFamily}", sans-serif`;
}
function measureFallbackText(ctx, model, initials, pixelSize) {
    ctx.font = resolveFallbackIconFont(model, pixelSize);
    if (typeof ctx.measureText !== 'function') {
        return {
            width: 0,
            height: pixelSize,
            ascent: pixelSize * 0.78,
            descent: pixelSize * 0.22,
            precise: false,
        };
    }
    const metrics = ctx.measureText(initials);
    const ascent = Number(metrics.actualBoundingBoxAscent);
    const descent = Number(metrics.actualBoundingBoxDescent);
    const left = Number(metrics.actualBoundingBoxLeft);
    const right = Number(metrics.actualBoundingBoxRight);
    const width = Number(metrics.width) || 0;
    if (Number.isFinite(ascent) && Number.isFinite(descent) && ascent + descent > 0) {
        const measuredWidth = Number.isFinite(left) && Number.isFinite(right) && left + right > 0 ? left + right : width;
        return {
            width: measuredWidth,
            height: ascent + descent,
            ascent,
            descent,
            x: Number.isFinite(left) ? -left : 0,
            y: -ascent,
            precise: true,
        };
    }
    return {
        width,
        height: pixelSize,
        ascent: pixelSize * 0.78,
        descent: pixelSize * 0.22,
        x: 0,
        y: pixelSize * -0.78,
        precise: false,
    };
}
/**
 * Lay out initials at a normalized type size with shrink only for overflow pairs.
 */
function fallbackInitialsSourceLayout(ctx, initials, model = {}) {
    const style = fallbackStyle(model);
    const sourceSize = Math.max(1, style.renderSize);
    const inset = Math.max(0, Math.min(sourceSize / 2, style.inset));
    const textSize = Math.max(0, sourceSize - inset * 2);
    const fontSize = Math.max(10, sourceSize * FALLBACK_ICON_FONT_SCALE);
    const bounds = measureFallbackText(ctx, model, initials, fontSize);
    if (!textSize || bounds.width <= 0 || bounds.height <= 0) {
        return {
            bounds,
            fontSize,
            offsetX: inset,
            offsetY: inset,
            scale: 1,
            sourceSize,
        };
    }
    const scale = Math.min(1, textSize / bounds.width, textSize / bounds.height);
    return {
        bounds,
        fontSize,
        offsetX: inset + (textSize - bounds.width * scale) / 2,
        offsetY: inset + (textSize - bounds.height * scale) / 2,
        scale,
        sourceSize,
    };
}
/**
 * Draw fallback initials into one square canvas icon footprint.
 */
export function drawFallbackInitialsCanvas(ctx, model, x, y, size, options = {}) {
    const initials = typeof model?.initials === 'string' && model.initials ? model.initials : '?';
    if (!ctx ||
        typeof ctx.fillText !== 'function' ||
        typeof ctx.save !== 'function' ||
        typeof ctx.restore !== 'function' ||
        typeof ctx.translate !== 'function' ||
        typeof ctx.scale !== 'function') {
        return;
    }
    const layout = fallbackInitialsSourceLayout(ctx, initials, model ?? {});
    const targetScale = size / layout.sourceSize;
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(targetScale, targetScale);
    ctx.translate(layout.offsetX, layout.offsetY);
    ctx.scale(layout.scale, layout.scale);
    ctx.translate(-(layout.bounds.x ?? 0), -(layout.bounds.y ?? 0));
    ctx.font = resolveFallbackIconFont(model, layout.fontSize);
    ctx.fillStyle = typeof options.color === 'string' ? options.color : '#ffffff';
    ctx.textAlign = 'left';
    ctx.textBaseline = 'alphabetic';
    ctx.fillText(initials, 0, 0);
    ctx.restore();
}
function resolveDevicePixelRatio() {
    const ratio = typeof globalThis !== 'undefined' ? Number(globalThis.devicePixelRatio || 1) : 1;
    return Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
}
/**
 * Render fallback initials into a DOM canvas using device-pixel-ratio backing.
 */
export function drawFallbackInitialsElementCanvas(canvas, model) {
    if (!canvas || typeof canvas.getContext !== 'function') {
        return;
    }
    const sourceSize = fallbackStyle(model).renderSize;
    const ratio = resolveDevicePixelRatio();
    canvas.width = Math.round(sourceSize * ratio);
    canvas.height = Math.round(sourceSize * ratio);
    canvas.style.width = '100%';
    canvas.style.height = '100%';
    let ctx = null;
    try {
        ctx = canvas.getContext('2d');
    }
    catch (_error) {
        return;
    }
    if (!ctx) {
        return;
    }
    if (typeof ctx.clearRect === 'function') {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
    }
    ctx.save();
    if (typeof ctx.scale === 'function') {
        ctx.scale(ratio, ratio);
    }
    drawFallbackInitialsCanvas(ctx, model, 0, 0, sourceSize);
    ctx.restore();
}
