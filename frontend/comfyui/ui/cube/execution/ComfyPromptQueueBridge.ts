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
/** Install one stable SugarCubes boundary around Comfy's prompt queue. */

type QueuePrompt = (position: number, payload: unknown) => Promise<unknown>;

export interface ComfyPromptQueueApi {
  queuePrompt?: QueuePrompt;
}

export interface ComfyPromptQueueBridgeOptions {
  api: ComfyPromptQueueApi;
  preflight?(): void;
  transform(payload: unknown): Promise<unknown>;
}

/** Own the sole SugarCubes host queue interaction across workflow lifecycles. */
export class ComfyPromptQueueBridge {
  readonly #api: ComfyPromptQueueApi;
  readonly #preflight: () => void;
  readonly #transform: (payload: unknown) => Promise<unknown>;
  #original: QueuePrompt | null = null;
  #installed = false;

  /** Bind one stable host API and one application-level prompt pipeline. */
  constructor(options: ComfyPromptQueueBridgeOptions) {
    this.#api = options.api;
    this.#preflight = options.preflight ?? (() => undefined);
    this.#transform = options.transform;
  }

  /** Install the bridge exactly once. */
  install(): void {
    if (this.#installed) return;
    const original = this.#api.queuePrompt;
    if (!original) throw new TypeError('Comfy queuePrompt is unavailable.');
    this.#original = original;
    this.#api.queuePrompt = this.#queueWrapper;
    this.#installed = true;
  }

  /** Restore the host queue only while this bridge remains its current owner. */
  dispose(): void {
    if (!this.#installed) return;
    if (this.#api.queuePrompt === this.#queueWrapper && this.#original) {
      this.#api.queuePrompt = this.#original;
    }
    this.#original = null;
    this.#installed = false;
  }

  /** Retain a stable wrapper identity so disposal cannot remove another hook. */
  readonly #queueWrapper: QueuePrompt = async (position, payload) => {
    const original = this.#original;
    if (!original) throw new Error('SugarCubes prompt queue bridge is not installed.');
    this.#preflight();
    const transformed = await this.#transform(payload);
    return await original.call(this.#api, position, transformed);
  };
}
