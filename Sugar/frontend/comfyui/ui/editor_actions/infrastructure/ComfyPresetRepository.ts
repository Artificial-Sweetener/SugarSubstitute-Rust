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
/** Adapt versioned presets to Comfy's atomic, per-user file storage. */
import type { ApiResponse } from '../../types/graph.js';
import type { UserPreset } from '../domain/UserPreset.js';
import { decodePresetDocument, encodePresetDocument } from '../domain/PresetCodec.js';
import type { UserPresetRepository } from '../application/UserPresetLibrary.js';
export type PresetApiFetch = (path: string, options?: RequestInit) => Promise<ApiResponse>;
const path = '/userdata/SugarCubes%2Fpresets.json';

/** Keep libraries portable across browsers by storing them with the current Comfy user. */
export class ComfyPresetRepository implements UserPresetRepository {
  readonly #fetch: PresetApiFetch;
  /** Bind the host's configured backend addressing and user headers. */
  constructor(fetchApi: PresetApiFetch) {
    this.#fetch = fetchApi;
  }
  /** Treat only a missing file as a fresh library; errors must never become an empty save. */
  async load(): Promise<readonly UserPreset[]> {
    const response = await this.#request();
    if (response.status === 404) return [];
    if (!response.ok)
      throw new Error(`Preset library could not be read (${String(response.status)}).`);
    return decodePresetDocument(await response.json());
  }
  /** Persist the validated common schema through Comfy's same-directory atomic replacement. */
  async save(presets: readonly UserPreset[]): Promise<void> {
    const response = await this.#request({
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(encodePresetDocument(presets)),
    });
    if (!response.ok)
      throw new Error(`Preset library could not be saved (${String(response.status)}).`);
  }
  /** Bound network IO and release the timeout after the host request settles. */
  async #request(options: RequestInit = {}): Promise<ApiResponse> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 10_000);
    try {
      return await this.#fetch(path, { ...options, signal: controller.signal });
    } finally {
      clearTimeout(timeout);
    }
  }
}
