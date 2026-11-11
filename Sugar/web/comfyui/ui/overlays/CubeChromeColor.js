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
 * Compute immutable Cube chrome colors, text measurements, and layout.
 */
function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
}
function parseHexChannel(value) {
    const parsed = Number.parseInt(value, 16);
    return Number.isFinite(parsed) ? parsed : null;
}
function parseColorToRgb(color) {
    if (typeof color !== 'string') {
        return null;
    }
    const hex = color.trim();
    if (!hex.startsWith('#')) {
        return null;
    }
    if (hex.length === 4) {
        const r = parseHexChannel((hex[1] ?? '') + (hex[1] ?? ''));
        const g = parseHexChannel((hex[2] ?? '') + (hex[2] ?? ''));
        const b = parseHexChannel((hex[3] ?? '') + (hex[3] ?? ''));
        if (r == null || g == null || b == null) {
            return null;
        }
        return { r, g, b };
    }
    if (hex.length === 7) {
        const r = parseHexChannel(hex.slice(1, 3));
        const g = parseHexChannel(hex.slice(3, 5));
        const b = parseHexChannel(hex.slice(5, 7));
        if (r == null || g == null || b == null) {
            return null;
        }
        return { r, g, b };
    }
    return null;
}
function rgbToHsl({ r, g, b }) {
    const rn = r / 255;
    const gn = g / 255;
    const bn = b / 255;
    const max = Math.max(rn, gn, bn);
    const min = Math.min(rn, gn, bn);
    const delta = max - min;
    let h = 0;
    let s = 0;
    const l = (max + min) / 2;
    if (delta !== 0) {
        if (max === rn) {
            h = ((gn - bn) / delta) % 6;
        }
        else if (max === gn) {
            h = (bn - rn) / delta + 2;
        }
        else {
            h = (rn - gn) / delta + 4;
        }
        h = Math.round(h * 60);
        if (h < 0) {
            h += 360;
        }
        s = delta / (1 - Math.abs(2 * l - 1));
    }
    return { h, s, l };
}
function hslToRgb({ h, s, l }) {
    const c = (1 - Math.abs(2 * l - 1)) * s;
    const hh = h / 60;
    const x = c * (1 - Math.abs((hh % 2) - 1));
    let r1 = 0;
    let g1 = 0;
    let b1 = 0;
    if (hh >= 0 && hh < 1) {
        r1 = c;
        g1 = x;
    }
    else if (hh >= 1 && hh < 2) {
        r1 = x;
        g1 = c;
    }
    else if (hh >= 2 && hh < 3) {
        g1 = c;
        b1 = x;
    }
    else if (hh >= 3 && hh < 4) {
        g1 = x;
        b1 = c;
    }
    else if (hh >= 4 && hh < 5) {
        r1 = x;
        b1 = c;
    }
    else {
        r1 = c;
        b1 = x;
    }
    const m = l - c / 2;
    return {
        r: Math.round((r1 + m) * 255),
        g: Math.round((g1 + m) * 255),
        b: Math.round((b1 + m) * 255),
    };
}
function rgbToHex({ r, g, b }) {
    const toHex = (value) => value.toString(16).padStart(2, '0');
    return `#${toHex(clamp(Math.round(r), 0, 255))}${toHex(clamp(Math.round(g), 0, 255))}${toHex(clamp(Math.round(b), 0, 255))}`;
}
/** Rotate a base color by the requested hue offset. */
export function triadicColor(baseColor, offset) {
    const rgb = parseColorToRgb(baseColor);
    if (!rgb) {
        return baseColor;
    }
    const hsl = rgbToHsl(rgb);
    const shifted = (hsl.h + offset + 360) % 360;
    const nextRgb = hslToRgb({ h: shifted, s: hsl.s, l: hsl.l });
    return rgbToHex(nextRgb);
}
