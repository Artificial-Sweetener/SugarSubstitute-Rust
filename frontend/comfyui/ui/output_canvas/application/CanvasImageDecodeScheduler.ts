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
/** Own two bounded decode admissions, finite deadlines and late-result cancellation. */

import type { ExecutionClock } from '../../execution/domain/WorkflowExecutionEvent.js';
import type {
  CanvasDecodedImage,
  CanvasImageInput,
  CanvasImageLoad,
  CanvasImageLoadPort,
} from '../domain/CanvasImageResource.js';

export interface CanvasDecodeLease {
  /** Cancel this exact admission without releasing another slot's decode. */
  cancel(): void;
}

export interface CanvasDecodeCallbacks<Handle> {
  /** Accept a completed owned handle; the slot owner must release it after display. */
  ready(image: CanvasDecodedImage<Handle>): void;
  /** Report a recoverable failure with its original exception context. */
  failed(error: unknown): void;
}

interface DecodeTask<Handle> {
  readonly load: CanvasImageLoad<Handle>;
  readonly deadline: () => void;
}

/** Keep transport cancellation separate from slot selection, byte budgets, and presentation. */
export class CanvasImageDecodeScheduler<Payload, Handle> {
  readonly #tasks = new Map<CanvasDecodeLease, DecodeTask<Handle>>();
  #disposed = false;

  /** Inject platform loading and elapsed time without exposing browser APIs to the policy owner. */
  constructor(
    private readonly port: CanvasImageLoadPort<Payload, Handle>,
    private readonly clock: ExecutionClock,
  ) {}

  /** Start only an available admission and dispose late completions after cancellation. */
  start(
    input: CanvasImageInput<Payload>,
    callbacks: CanvasDecodeCallbacks<Handle>,
  ): CanvasDecodeLease | null {
    if (this.#disposed || this.#tasks.size >= 2) return null;
    const load = this.port.start(input);
    const lease: CanvasDecodeLease = Object.freeze({
      cancel: () => {
        this.#cancel(lease);
      },
    });
    let deadline: () => void;
    try {
      deadline = this.clock.later(10000, () => {
        if (!this.#cancel(lease)) return;
        callbacks.failed(new Error('Canvas image loading timed out.'));
      });
    } catch (error: unknown) {
      load.cancel();
      // No lease is returned on deadline setup failure; consume cancellation and rethrow its cause.
      void load.completion.catch(() => {});
      throw error;
    }
    this.#tasks.set(lease, { load, deadline });
    void load.completion.then(
      (image) => {
        const task = this.#tasks.get(lease);
        if (!task) {
          image.release();
          return;
        }
        this.#tasks.delete(lease);
        task.deadline();
        try {
          callbacks.ready(image);
        } catch (error: unknown) {
          image.release();
          callbacks.failed(error);
        }
      },
      (error: unknown) => {
        const task = this.#tasks.get(lease);
        if (!task) return;
        this.#tasks.delete(lease);
        task.deadline();
        task.load.cancel();
        callbacks.failed(error);
      },
    );
    return lease;
  }

  /** Count only owned platform admissions, excluding browser-controlled cached decode work. */
  get pending(): number {
    return this.#tasks.size;
  }

  /** Cancel all owned handles, deadlines and future delivery at attachment teardown. */
  dispose(): void {
    this.#disposed = true;
    for (const lease of [...this.#tasks.keys()]) this.#cancel(lease);
  }

  /** Release this exact active lease once; foreign/stale leases cannot affect another admission. */
  #cancel(lease: CanvasDecodeLease): boolean {
    const task = this.#tasks.get(lease);
    if (!task) return false;
    this.#tasks.delete(lease);
    task.deadline();
    task.load.cancel();
    return true;
  }
}
