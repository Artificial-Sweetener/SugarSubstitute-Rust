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
/** Adapt Comfy canvas coordinates into graph-space Cube placement origins. */
import { readVector2 } from '../graph/VectorUtils.js';
/** Own fallback ordering for pointer, viewport-center, and transform origins. */
export class ComfyCanvasDropOriginAdapter {
    #getCanvas;
    #logger;
    /** Bind the dynamic canvas lookup used across workflow replacements. */
    constructor(options) {
        this.#getCanvas = options.getCanvas;
        this.#logger = options.logger;
    }
    /** Compute one finite graph-space placement origin. */
    compute() {
        const canvas = this.#getCanvas();
        if (!canvas)
            return [0, 0];
        const lastMouse = canvas.last_mouse_position;
        if (Array.isArray(lastMouse) &&
            Number.isFinite(lastMouse[0]) &&
            Number.isFinite(lastMouse[1])) {
            const converted = this.#convert(canvas, lastMouse);
            if (converted)
                return converted;
        }
        try {
            const rect = canvas.canvas?.getBoundingClientRect?.();
            if (rect) {
                const converted = this.#convert(canvas, [rect.width / 2, rect.height / 2]);
                if (converted)
                    return converted;
            }
        }
        catch (error) {
            this.#logger.warn('SugarCubes: viewport-center conversion failed.', error);
        }
        const offset = Array.isArray(canvas.ds?.offset) ? canvas.ds.offset : [0, 0];
        const origin = [-Number(offset[0] ?? 0), -Number(offset[1] ?? 0)];
        return origin.every(Number.isFinite) ? origin : [0, 0];
    }
    /** Convert one canvas-relative point through Comfy's public transform surface. */
    #convert(canvas, point) {
        if (!Array.isArray(point))
            return null;
        try {
            if (typeof canvas.convertCanvasToOffset === 'function') {
                const converted = canvas.convertCanvasToOffset(readVector2(point, 0, 0));
                if (Array.isArray(converted) && converted.length >= 2) {
                    const value = [Number(converted[0]), Number(converted[1])];
                    if (value.every(Number.isFinite))
                        return value;
                }
            }
            const scale = Number(canvas.ds?.scale) || 1;
            const offset = Array.isArray(canvas.ds?.offset) ? canvas.ds.offset : [0, 0];
            const value = [
                Number(point[0] ?? 0) / scale - Number(offset[0] ?? 0),
                Number(point[1] ?? 0) / scale - Number(offset[1] ?? 0),
            ];
            return value.every(Number.isFinite) ? value : null;
        }
        catch (error) {
            this.#logger.warn('SugarCubes: canvas-point conversion failed.', error);
            return null;
        }
    }
}
