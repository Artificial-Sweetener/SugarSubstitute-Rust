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
/** Load the bundled model-target vocabulary for Cube authoring. */
import { normalizeSupportedModels } from './ModelTargets.js';

export type CubeModelSuggestionFetch = (
  url: string,
  options: RequestInit,
) => Promise<{ ok: boolean; text(): Promise<string> }>;

const MODEL_LIST_URLS = [
  new URL('../../../models.txt', import.meta.url).toString(),
  '/extensions/ComfyUI-SugarCubes/models.txt',
];

/** Own the bundled model-target vocabulary used by authoring and metadata editors. */
export class CubeModelSuggestions {
  #values: readonly string[] = ['Other'];
  #loading: Promise<void> | null = null;

  /** Bind the host fetch adapter and actionable diagnostics. */
  constructor(
    private readonly fetch: CubeModelSuggestionFetch | null,
    private readonly logger: Pick<Console, 'warn'>,
  ) {}

  /** Return a copy so editable dialog values cannot mutate the vocabulary. */
  getModelSuggestions(): readonly string[] {
    return this.#values.slice();
  }

  /** Load once through both supported extension URLs with bounded requests. */
  setup(): Promise<void> {
    return (this.#loading ??= this.#load());
  }

  /** Retain a usable fallback when the bundled resource is unavailable. */
  async #load(): Promise<void> {
    if (!this.fetch) return;
    for (const url of MODEL_LIST_URLS) {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 15000);
      try {
        const response = await this.fetch(url, { signal: controller.signal });
        if (!response.ok) continue;
        const values = normalizeSupportedModels(
          (await response.text())
            .split(/\r?\n/)
            .map((value) => value.trim())
            .filter(Boolean),
        );
        if (values.length) this.#values = values;
        return;
      } catch (error: unknown) {
        this.logger.warn('SugarCubes model suggestions request failed.', { url, error });
      } finally {
        clearTimeout(timeout);
      }
    }
  }
}
