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
/** Distinguish temporary installed-asset transport failures from incompatible native components. */

/** Preserve the request context and underlying network failure for recovery and diagnostics. */
export class ComfyRuntimeAssetUnavailableError extends Error {
  /** Identify one unavailable installed asset without misclassifying renderer compatibility. */
  constructor(
    readonly url: string,
    cause: unknown,
  ) {
    super('Waiting for the ComfyUI connection to load native controls.', { cause });
    this.name = 'ComfyRuntimeAssetUnavailableError';
  }
}

/** Receive only the browser response fields required to classify an installed asset request. */
export type ComfyRuntimeAssetRequester = (
  url: string,
  options: { signal: AbortSignal },
) => Promise<Pick<Response, 'ok' | 'status' | 'text'>>;

/** Fetch installed module text with a bounded request and explicit transport classification. */
export async function fetchComfyRuntimeText(
  url: string,
  request: ComfyRuntimeAssetRequester = (path, options) => fetch(path, options),
): Promise<string> {
  const controller = new AbortController();
  const timeout = globalThis.setTimeout(() => controller.abort(), 5_000);
  try {
    const response = await request(url, { signal: controller.signal });
    if (!response.ok) {
      const failure = new Error(
        `Comfy runtime asset request failed with status ${response.status}.`,
      );
      if (response.status === 408 || response.status === 429 || response.status >= 500)
        throw new ComfyRuntimeAssetUnavailableError(url, failure);
      throw failure;
    }
    return await response.text();
  } catch (error: unknown) {
    if (
      error instanceof TypeError ||
      (error instanceof DOMException && error.name === 'AbortError')
    )
      throw new ComfyRuntimeAssetUnavailableError(url, error);
    throw error;
  } finally {
    globalThis.clearTimeout(timeout);
  }
}

/** Recognize browser module transport failures without retrying invalid imports or evaluated code. */
export function isComfyModuleTransportFailure(error: unknown): boolean {
  return (
    error instanceof TypeError &&
    /failed to fetch|error loading dynamically imported module|importing a module script failed/i.test(
      error.message,
    )
  );
}
