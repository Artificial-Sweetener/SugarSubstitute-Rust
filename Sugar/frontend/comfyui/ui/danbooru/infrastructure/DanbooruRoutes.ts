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
/**
 * SugarCubes' Danbooru route, as the prompt editor's paste import asks it. The backend talks to
 * Danbooru; the page never reaches Danbooru itself.
 */
import type { BooruLookupKey } from '../../../../sugarprompt/services/src/booru/links.js';

/** The part of a fetch response this adapter reads. */
export interface DanbooruResponse {
  readonly ok?: boolean;
  readonly status?: number;
  json(): Promise<unknown>;
}
export type DanbooruFetch = (path: string, options?: RequestInit) => Promise<DanbooruResponse>;

/** Ask SugarCubes' Danbooru route; every request has a time limit and can be cancelled. */
export class DanbooruRoutes {
  constructor(private readonly fetchApi: DanbooruFetch) {}

  /**
   * The post a pasted link names, in the shape the editor's paste import validates. A backend
   * that cannot be reached is Danbooru being unavailable, so the link is still pasted.
   */
  async pastedPost(key: BooruLookupKey, signal: AbortSignal): Promise<unknown> {
    const query: Record<string, string> = key.kind === 'post' ? { id: key.id } : { md5: key.md5 };
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30_000);
    const abort = (): void => controller.abort();
    signal.addEventListener('abort', abort, { once: true });
    try {
      const response = await this.fetchApi(
        `/sugarcubes/v1/danbooru/pasted-post?${new URLSearchParams(query).toString()}`,
        { signal: controller.signal },
      );
      return response.ok ? await response.json() : { status: 'unavailable' };
    } catch (error: unknown) {
      if (signal.aborted) throw error;
      return { status: 'unavailable' };
    } finally {
      clearTimeout(timeout);
      signal.removeEventListener('abort', abort);
    }
  }
}
