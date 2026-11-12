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
/** Apply dotted proximity routing after Comfy compiles native Cube nodes. */

export interface CubePromptPipelineOptions {
  applyProximity(payload: unknown): unknown;
  adaptCubeOutputs(payload: unknown): unknown;
}

/** Own the ordered application workflow for one outgoing Comfy prompt. */
export class CubePromptPipeline {
  readonly #applyProximity: (payload: unknown) => unknown;
  readonly #adaptCubeOutputs: (payload: unknown) => unknown;

  /** Bind renderer-neutral proximity policy after native graph compilation. */
  constructor(options: CubePromptPipelineOptions) {
    this.#applyProximity = options.applyProximity;
    this.#adaptCubeOutputs = options.adaptCubeOutputs;
  }

  /** Preserve Comfy's native payload and apply current proximity matches. */
  async transform(payload: unknown): Promise<unknown> {
    return this.#adaptCubeOutputs(this.#applyProximity(payload));
  }
}
