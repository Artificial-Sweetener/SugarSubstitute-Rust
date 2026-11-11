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
/** Coalesce selection-scoped Cube version discovery. */

import type { CubeVersionOption } from './CubeVersionTypes.js';

export interface CubeVersionOptionSource {
  listOptions(cubeId: string, fallbackVersion: string): Promise<CubeVersionOption[]>;
}

/** Cache immutable version choices while one Cube library identity is unchanged. */
export class CubeVersionAvailabilityService {
  readonly #repository: CubeVersionOptionSource;
  readonly #requests = new Map<string, Promise<readonly CubeVersionOption[]>>();

  /** Bind the authoritative version repository. */
  constructor(repository: CubeVersionOptionSource) {
    this.#repository = repository;
  }

  /** Return one shared request for a Cube's unique semantic versions. */
  list(cubeId: string, currentVersion: string): Promise<readonly CubeVersionOption[]> {
    const key = `${cubeId.trim()}\u0000${currentVersion.trim()}`;
    const existing = this.#requests.get(key);
    if (existing) return existing;
    const request = this.#repository
      .listOptions(cubeId, currentVersion)
      .then((options) => Object.freeze([...options]))
      .catch((error: unknown) => {
        this.#requests.delete(key);
        throw error;
      });
    this.#requests.set(key, request);
    return request;
  }

  /** Invalidate cached history after a library mutation. */
  invalidate(cubeId?: string): void {
    const normalized = cubeId?.trim();
    if (!normalized) {
      this.#requests.clear();
      return;
    }
    for (const key of this.#requests.keys()) {
      if (key.startsWith(`${normalized}\u0000`)) this.#requests.delete(key);
    }
  }
}
