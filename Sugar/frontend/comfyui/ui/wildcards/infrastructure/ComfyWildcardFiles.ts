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
/** Reach the user's wildcard folder through Comfy's per-user file storage. */
import { isRecord } from '../../types/common.js';
import { wildcardFile, type WildcardFile } from '../domain/WildcardSource.js';
import type { WildcardFileStore } from '../application/WildcardLibrary.js';
import {
  ComfyUserFolder,
  type UserFileFetch,
  type UserFileResponse,
} from '../../core/user_files/ComfyUserFolder.js';

/** The wildcard folder inside the Comfy user's own folder: `user/<user>/sugar/wildcards`. */
export const WILDCARD_FOLDER = 'sugar/wildcards';

export type WildcardApiResponse = UserFileResponse;
export type WildcardApiFetch = UserFileFetch;

/** List, read, create, save, rename and delete wildcard files, and open their folder. */
export class ComfyWildcardFiles implements WildcardFileStore {
  readonly #folder: ComfyUserFolder;

  /** Bind the host's configured backend fetch. */
  constructor(private readonly fetchApi: WildcardApiFetch) {
    this.#folder = new ComfyUserFolder(fetchApi, WILDCARD_FOLDER, 'wildcard file');
  }

  /** List every wildcard file; a folder that does not exist yet holds none. */
  async list(): Promise<readonly WildcardFile[]> {
    return (await this.#folder.list()).flatMap((entry) => {
      const file = wildcardFile(entry.path, entry.stamp);
      return file ? [file] : [];
    });
  }

  /** Read one file's text; null when it no longer exists. */
  read(path: string): Promise<string | null> {
    return this.#folder.read(path);
  }

  /** Create a file with the given text, never replacing one that exists. */
  create(path: string, text: string): Promise<'created' | 'exists'> {
    return this.#folder.create(path, text);
  }

  /** Replace a file's text, creating the file when it does not exist. */
  write(path: string, text: string): Promise<void> {
    return this.#folder.write(path, text);
  }

  /** Rename a file inside the folder, never replacing one that exists. */
  rename(from: string, to: string): Promise<'renamed' | 'exists' | 'missing'> {
    return this.#folder.rename(from, to);
  }

  /** Delete a file; one that is already gone counts as deleted. */
  delete(path: string): Promise<void> {
    return this.#folder.delete(path);
  }

  /** Show the folder in the file manager of the machine ComfyUI runs on, creating it if needed. */
  async openFolder(): Promise<void> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);
    let response: WildcardApiResponse;
    try {
      response = await this.fetchApi('/sugarcubes/wildcards/open_folder', {
        method: 'POST',
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timeout);
    }
    if (response.ok) return;
    const body: unknown = await response.json().catch(() => null);
    const error = isRecord(body) && isRecord(body.error) ? body.error.message : null;
    throw new Error(
      typeof error === 'string' && error ? error : 'The wildcard folder could not be opened.',
    );
  }
}
