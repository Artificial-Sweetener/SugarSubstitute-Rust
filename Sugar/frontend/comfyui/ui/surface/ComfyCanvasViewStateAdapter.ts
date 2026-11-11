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
/** Adapt Comfy's mutable canvas transform to typed Cube navigation state. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

export interface CubeCanvasViewState {
  scale: number;
  offset: readonly [number, number];
}

/** Own validated reads and writes at Comfy's dynamic canvas-view boundary. */
export class ComfyCanvasViewStateAdapter {
  readonly #canvas: UnknownRecord;
  readonly #setDirtyCanvas: (foreground?: boolean, background?: boolean) => void;
  readonly #logger: Pick<Console, 'debug' | 'warn'> | null;

  /** Bind one live Comfy canvas and its repaint request. */
  constructor(
    canvas: UnknownRecord,
    setDirtyCanvas: (foreground?: boolean, background?: boolean) => void,
    logger: Pick<Console, 'debug' | 'warn'> | null = null,
  ) {
    this.#canvas = canvas;
    this.#setDirtyCanvas = setDirtyCanvas;
    this.#logger = logger;
  }

  /** Capture one finite canvas transform, if Comfy currently exposes it. */
  capture(): CubeCanvasViewState | null {
    const transform = isRecord(this.#canvas.ds) ? this.#canvas.ds : null;
    const offset = readMutableOffset(transform?.offset);
    const scale = Number(transform?.scale);
    const x = Number(offset?.['0']);
    const y = Number(offset?.['1']);
    if (!Number.isFinite(scale) || scale <= 0 || !Number.isFinite(x) || !Number.isFinite(y)) {
      this.#logger?.warn('SugarCubes could not capture the current Comfy canvas viewport.');
      return null;
    }
    this.#logger?.debug(
      `SugarCubes captured the surface viewport at scale ${String(scale)} and offset ${String(x)},${String(y)}.`,
    );
    return { scale, offset: [x, y] };
  }

  /** Restore one captured transform without assuming ownership of the graph. */
  restore(state: CubeCanvasViewState): void {
    const transform = isRecord(this.#canvas.ds) ? this.#canvas.ds : null;
    const offset = readMutableOffset(transform?.offset);
    if (!transform || !offset) return;
    transform.scale = state.scale;
    offset['0'] = state.offset[0];
    offset['1'] = state.offset[1];
    this.#setDirtyCanvas(true, true);
    this.#logger?.debug(
      `SugarCubes restored the surface viewport at scale ${String(state.scale)} and offset ${String(state.offset[0])},${String(state.offset[1])}.`,
    );
  }
}

/** Narrow Comfy's Array or typed-array offset to its mutable numeric surface. */
function readMutableOffset(value: unknown): UnknownRecord | null {
  if (!isRecord(value)) return null;
  return Number.isFinite(Number(value['0'])) && Number.isFinite(Number(value['1'])) ? value : null;
}
