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
/** Advance animation time explicitly and count pending frame ownership. */

import type { CanvasFrameClock } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasCameraTransition.js';

/** Execute one scheduled frame batch at a deterministic monotonic timestamp. */
export class CanvasFrameFixture implements CanvasFrameClock {
  #time = 0;
  readonly #frames = new Set<(time: number) => void>();
  /** Read monotonic elapsed milliseconds. */
  now(): number {
    return this.#time;
  }
  /** Retain one cancellable frame without any implicit idle ticking. */
  request(callback: (time: number) => void): () => void {
    this.#frames.add(callback);
    return () => this.#frames.delete(callback);
  }
  /** Advance only the callbacks pending at the start of this simulated frame. */
  advance(milliseconds: number): void {
    this.#time += milliseconds;
    const frames = [...this.#frames];
    this.#frames.clear();
    for (const frame of frames) frame(this.#time);
  }
  /** Count retained animation callbacks for lifecycle proofs. */
  get pending(): number {
    return this.#frames.size;
  }
}
