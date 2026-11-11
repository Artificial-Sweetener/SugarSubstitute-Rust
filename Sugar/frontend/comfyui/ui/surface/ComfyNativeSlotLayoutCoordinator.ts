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
/** Coalesce Cube boundary changes into Comfy's native slot-layout synchronization. */

import type { ComfyVueRuntime } from './ComfyRuntimeModuleLoader.js';

export interface ComfyNativeSlotLayoutCoordinatorOptions {
  getRuntime(): Promise<ComfyVueRuntime>;
  requestFrame(callback: FrameRequestCallback): number | null;
  cancelFrame(handle: number): void;
  logger: Pick<Console, 'error'>;
}

/** Own the one graph-wide host call required after a batch of Cube socket movement. */
export class ComfyNativeSlotLayoutCoordinator {
  readonly #getRuntime: () => Promise<ComfyVueRuntime>;
  readonly #requestFrame: (callback: FrameRequestCallback) => number | null;
  readonly #cancelFrame: (handle: number) => void;
  readonly #logger: Pick<Console, 'error'>;
  #frame: number | null = null;
  #disposed = false;

  /** Bind Comfy's lazy runtime and the active document scheduler. */
  constructor(options: ComfyNativeSlotLayoutCoordinatorOptions) {
    this.#getRuntime = options.getRuntime;
    this.#requestFrame = options.requestFrame;
    this.#cancelFrame = options.cancelFrame;
    this.#logger = options.logger;
  }

  /** Request one native synchronization for all boundary changes in the current frame. */
  request(): void {
    if (this.#disposed || this.#frame !== null) return;
    let invokedSynchronously = false;
    const frame = this.#requestFrame(() => {
      invokedSynchronously = true;
      this.#frame = null;
      void this.#synchronize();
    });
    if (frame !== null && !invokedSynchronously) {
      this.#frame = frame;
      return;
    }
    if (frame === null) void this.#synchronize();
  }

  /** Cancel pending work and ignore a runtime that resolves after presentation disposal. */
  dispose(): void {
    this.#disposed = true;
    if (this.#frame !== null) this.#cancelFrame(this.#frame);
    this.#frame = null;
  }

  /** Invoke the guarded native capability while preserving actionable failure context. */
  async #synchronize(): Promise<void> {
    try {
      const runtime = await this.#getRuntime();
      if (!this.#disposed) runtime.requestSlotLayoutSync();
    } catch (error: unknown) {
      if (this.#disposed) return;
      this.#logger.error('SugarCubes could not synchronize native Cube boundary slots.', {
        reason: error instanceof Error ? error.message : String(error),
        error,
      });
    }
  }
}
