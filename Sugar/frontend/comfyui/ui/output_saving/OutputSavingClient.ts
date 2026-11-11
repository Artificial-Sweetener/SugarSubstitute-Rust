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
/** Address the authoritative output-saving service without storing browser-owned preference copies. */
import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import { isRecord } from '../types/common.js';
import {
  decodeOutputSavingSnapshot,
  type OutputSavingPreferences,
  type OutputSavingSnapshot,
} from './OutputSavingPreferences.js';

/** Expose bounded commands independently of settings rendering and the canvas. */
export class OutputSavingClient {
  /** Bind the established host-addressed JSON transport. */
  constructor(private readonly api: Pick<CubeLibraryApi, 'fetchJson'>) {}

  /** Read validated saving policy without changing user preferences. */
  load(signal?: AbortSignal): Promise<OutputSavingSnapshot> {
    return this.#request('settings', undefined, signal);
  }

  /** Commit a complete deliberate settings edit before subsequent runs capture it. */
  save(preferences: OutputSavingPreferences): Promise<OutputSavingSnapshot> {
    return this.#request('settings', preferences);
  }

  /** Render a proposed path without saving the draft or allocating output files. */
  preview(
    preferences: OutputSavingPreferences,
    signal: AbortSignal,
  ): Promise<OutputSavingSnapshot> {
    return this.#request('preview', preferences, signal);
  }

  /** Import explicit source bytes without reading or modifying the user's Substitute settings file. */
  importSubstitute(value: unknown): Promise<OutputSavingSnapshot> {
    return this.#request('import', value);
  }

  /** Bound network lifetime and preserve actionable literal server errors for SugarCubes feedback. */
  async #request(
    action: string,
    value?: unknown,
    signal?: AbortSignal,
  ): Promise<OutputSavingSnapshot> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal?.aborted) abort();
    signal?.addEventListener('abort', abort, { once: true });
    const timeout = setTimeout(abort, 20000);
    try {
      const options: RequestInit = { signal: controller.signal };
      if (value !== undefined) {
        options.method = 'POST';
        options.headers = { 'Content-Type': 'application/json' };
        options.body = JSON.stringify(value);
      }
      const { response, data } = await this.api.fetchJson(
        '/sugarcubes/output-saving/' + action,
        options,
      );
      if (response.ok === false || data.success !== true) {
        const error = isRecord(data.error) ? data.error.message : undefined;
        throw new Error(
          typeof error === 'string' ? error : 'Output saving settings are unavailable.',
        );
      }
      return decodeOutputSavingSnapshot(data.data);
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener('abort', abort);
    }
  }
}
