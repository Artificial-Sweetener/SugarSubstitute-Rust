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
/** Let the desktop body wash show through Comfy's graph while keeping Comfy's own grid lines. */
import { isRecord } from '../../types/common.js';
/** LiteGraph fields Comfy's palette loader assigns on every palette or background change. */
const CLEAR_COLOR = 'clear_background_color';
const GRID_IMAGE = 'background_image';
const GRID_PATTERN = '_pattern';
/**
 * Re-express an opaque grid tile as lines alone: each pixel becomes white or black with the alpha
 * that reproduces its original color over the tile's dominant field color, so the field itself
 * becomes fully transparent and line contrast survives over any wash.
 */
export function translucentGridPixels(pixels) {
    const at = (index) => pixels[index] ?? 0;
    const counts = new Map();
    for (let index = 0; index + 3 < pixels.length; index += 4) {
        const key = (at(index) << 16) | (at(index + 1) << 8) | at(index + 2);
        counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    let field = 0;
    let fieldCount = -1;
    for (const [key, count] of counts)
        if (count > fieldCount) {
            field = key;
            fieldCount = count;
        }
    const base = [(field >> 16) & 255, (field >> 8) & 255, field & 255];
    const output = new Uint8ClampedArray(pixels.length);
    for (let index = 0; index + 3 < pixels.length; index += 4) {
        let lighter = 0;
        let darker = 0;
        base.forEach((fieldChannel, channel) => {
            const delta = at(index + channel) - fieldChannel;
            if (delta > 0)
                lighter = Math.max(lighter, delta / Math.max(1, 255 - fieldChannel));
            else
                darker = Math.max(darker, -delta / Math.max(1, fieldChannel));
        });
        const white = lighter >= darker;
        const alpha = (white ? lighter : darker) * (at(index + 3) / 255);
        output[index] = output[index + 1] = output[index + 2] = white ? 255 : 0;
        output[index + 3] = Math.round(alpha * 255);
    }
    return output;
}
/** Decode a grid tile in the page and encode its translucent replacement as a PNG data URL. */
export function browserGridTileDeriver(document) {
    return (source) => new Promise((resolve, reject) => {
        const image = new Image();
        image.addEventListener('error', () => reject(new Error('Comfy grid tile did not decode.')));
        image.addEventListener('load', () => {
            const canvas = document.createElement('canvas');
            canvas.width = image.naturalWidth;
            canvas.height = image.naturalHeight;
            const context = canvas.getContext('2d');
            if (!context || canvas.width === 0 || canvas.height === 0) {
                reject(new Error('Comfy grid tile has no drawable raster.'));
                return;
            }
            context.drawImage(image, 0, 0);
            const tile = context.getImageData(0, 0, canvas.width, canvas.height);
            tile.data.set(translucentGridPixels(tile.data));
            context.putImageData(tile, 0, 0);
            resolve(canvas.toDataURL('image/png'));
        });
        image.src = source;
    });
}
/**
 * Own LiteGraph's background projection for one desktop frame lifetime. Comfy keeps assigning its
 * palette values; this owner records them and presents a transparent clear color plus a
 * translucent copy of Comfy's grid, so the CSS body wash beneath the graph canvas carries Mica.
 * Detaching restores Comfy's latest assignments as plain data properties.
 */
export class DesktopGraphBackdrop {
    options;
    #canvas = null;
    #comfy = { clear: undefined, image: undefined };
    #grids = new Map();
    #pending = new Set();
    constructor(options) {
        this.options = options;
    }
    /** Bind the current Comfy canvas once; repeated calls with the same canvas are idempotent. */
    attach(app) {
        const canvas = isRecord(app) && isRecord(app.canvas) ? app.canvas : null;
        if (canvas === this.#canvas)
            return;
        this.detach();
        if (!canvas)
            return;
        this.#canvas = canvas;
        this.#comfy = { clear: canvas[CLEAR_COLOR], image: canvas[GRID_IMAGE] };
        Object.defineProperty(canvas, CLEAR_COLOR, {
            configurable: true,
            enumerable: true,
            get: () => 'transparent',
            set: (value) => {
                this.#comfy.clear = value;
            },
        });
        Object.defineProperty(canvas, GRID_IMAGE, {
            configurable: true,
            enumerable: true,
            get: () => this.#presentedGrid(),
            set: (value) => {
                this.#comfy.image = value;
                this.#prepare(value);
            },
        });
        this.#prepare(this.#comfy.image);
        this.#redraw();
    }
    /** Return Comfy's own background values to its canvas without discarding later assignments. */
    detach() {
        const canvas = this.#canvas;
        if (!canvas)
            return;
        this.#canvas = null;
        for (const [name, value] of [
            [CLEAR_COLOR, this.#comfy.clear],
            [GRID_IMAGE, this.#comfy.image],
        ])
            Object.defineProperty(canvas, name, {
                configurable: true,
                enumerable: true,
                writable: true,
                value,
            });
        this.#redrawCanvas(canvas);
    }
    /** Show only a derived translucent tile; Comfy's opaque tile would hide the wash. */
    #presentedGrid() {
        const source = this.#comfy.image;
        return typeof source === 'string' ? (this.#grids.get(source) ?? null) : null;
    }
    #prepare(source) {
        if (typeof source !== 'string' || !source || this.#grids.has(source))
            return;
        if (this.#pending.has(source))
            return;
        this.#pending.add(source);
        this.options.deriveGrid(source).then((grid) => {
            this.#pending.delete(source);
            this.#grids.set(source, grid);
            if (this.#comfy.image === source)
                this.#redraw();
        }, (error) => {
            this.#pending.delete(source);
            this.options.report(error);
        });
    }
    #redraw() {
        if (this.#canvas)
            this.#redrawCanvas(this.#canvas);
    }
    /** Drop LiteGraph's cached pattern so the next background pass rebuilds it from the tile. */
    #redrawCanvas(canvas) {
        canvas[GRID_PATTERN] = undefined;
        const setDirty = canvas.setDirty;
        if (typeof setDirty === 'function')
            setDirty.call(canvas, true, true);
    }
}
