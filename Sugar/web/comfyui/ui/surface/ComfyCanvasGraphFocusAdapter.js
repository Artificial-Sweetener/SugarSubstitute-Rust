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
/** Focus a Comfy graph viewport through its host-owned transform API. */
import { isRecord } from '../types/common.js';
/** Own safe viewport framing at the dynamic Comfy transform boundary. */
export class ComfyCanvasGraphFocusAdapter {
    #canvas;
    #setDirtyCanvas;
    /** Bind the active Comfy canvas and its repaint request. */
    constructor(canvas, setDirtyCanvas) {
        this.#canvas = canvas;
        this.#setDirtyCanvas = setDirtyCanvas;
    }
    /** Frame one finite graph-space rectangle using Comfy's native viewport policy. */
    focus(bounds) {
        if (!bounds.every(Number.isFinite))
            return false;
        const transform = isRecord(this.#canvas.ds) ? this.#canvas.ds : null;
        const animateToBounds = transform?.animateToBounds;
        const fitToBounds = transform?.fitToBounds;
        const redraw = () => this.#setDirtyCanvas(true, true);
        if (typeof animateToBounds === 'function') {
            animateToBounds.call(transform, [...bounds], redraw);
        }
        else if (typeof fitToBounds === 'function') {
            fitToBounds.call(transform, [...bounds]);
            redraw();
        }
        else {
            return false;
        }
        return true;
    }
}
