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
/** Validate shared media policy and one-time image bytes at the HTTP adapter boundary. */
import type { OutputFileReference } from '../execution/domain/WorkflowOutput.js';
import type { OutputCopyFormat, OutputMediaProfile } from './domain/OutputMedia.js';
import { isRecord } from '../types/common.js';

export interface OutputMediaImage {
  readonly blob: Blob;
  readonly filename: string;
  readonly mimeType: 'image/png' | 'image/jpeg';
}

/** Keep bounded HTTP media operations reusable across canvas renderers. */
export class OutputMediaClient {
  /** Bind one same-origin transport and the actual browser connection address. */
  constructor(
    private readonly origin: string,
    private readonly fetch: typeof globalThis.fetch,
  ) {}

  /** Trust reveal only when both the browser origin and server's guarded capability are local. */
  async profile(): Promise<OutputMediaProfile> {
    return this.#request('/sugarcubes/output-saving/media-profile', async (response) => {
      const value: unknown = await response.json();
      if (
        !response.ok ||
        !isRecord(value) ||
        value.success !== true ||
        !isRecord(value.data) ||
        (value.data.preferred_copy_format !== 'png' &&
          value.data.preferred_copy_format !== 'jpeg') ||
        typeof value.data.reveal !== 'boolean' ||
        typeof value.data.photoshop !== 'boolean'
      )
        throw new Error('The output image action capabilities are unavailable.');
      const hostname = new URL(this.origin).hostname;
      const local =
        hostname === 'localhost' || hostname === '[::1]' || /^127(?:\.\d{1,3}){3}$/.test(hostname);
      return Object.freeze({
        preferredCopyFormat: value.data.preferred_copy_format,
        fileAction: local && value.data.reveal ? 'reveal' : 'download',
        photoshop: local && value.data.photoshop,
      });
    });
  }

  /** Fetch a full-resolution PNG or JPEG rendition without creating a permanent companion. */
  async image(
    file: OutputFileReference,
    format: OutputCopyFormat,
    signal?: AbortSignal,
  ): Promise<OutputMediaImage> {
    return this.#request(
      '/sugarcubes/output-saving/image',
      async (response) => {
        if (!response.ok)
          throw await responseError(response, 'The image could not be prepared for copying.');
        const blob = await response.blob();
        const encoded = response.headers.get('X-SugarCubes-Filename');
        const filename = encoded === null ? '' : decodeURIComponent(encoded);
        if (
          !blob.size ||
          (blob.type !== 'image/png' && blob.type !== 'image/jpeg') ||
          !filename ||
          /[/\\:\0]/.test(filename) ||
          !(blob.type === 'image/png' ? /\.png$/i.test(filename) : /\.jpe?g$/i.test(filename)) ||
          (format === 'png' && blob.type !== 'image/png') ||
          (format === 'jpeg' && blob.type !== 'image/jpeg')
        )
          throw new Error('The output image rendition response is invalid.');
        return { blob, filename, mimeType: blob.type };
      },
      { file, format },
      signal,
    );
  }

  /** Request the local adapter to reveal a validated file rather than accepting arbitrary paths. */
  async reveal(file: OutputFileReference, signal?: AbortSignal): Promise<void> {
    return this.#request(
      '/sugarcubes/output-saving/reveal',
      async (response) => {
        const value: unknown = await response.json();
        if (!response.ok || !isRecord(value) || value.success !== true)
          throw new Error(
            isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
              ? value.error.message
              : 'The image could not be revealed in its folder.',
          );
      },
      file,
      signal,
    );
  }

  /**
   * Ask the local adapter to open validated output images in Photoshop: one document named
   * `document` holding each image as a layer named by its entry, in order.
   */
  async photoshop(
    document: string,
    layers: readonly { readonly file: OutputFileReference; readonly name: string }[],
    signal?: AbortSignal,
  ): Promise<void> {
    return this.#request(
      '/sugarcubes/output-saving/photoshop',
      async (response) => {
        const value: unknown = await response.json();
        if (!response.ok || !isRecord(value) || value.success !== true)
          throw new Error(
            isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
              ? value.error.message
              : 'The images could not be opened in Photoshop.',
          );
      },
      { document, layers },
      signal,
    );
  }

  /** Build a same-origin download URL whose response supplies the selected rendition's filename. */
  downloadUrl(file: OutputFileReference): string {
    const url = new URL('/sugarcubes/output-saving/image', this.origin);
    if (url.protocol !== 'http:' && url.protocol !== 'https:')
      throw new Error('The image host is unavailable.');
    url.search = new URLSearchParams({ ...file, format: 'preferred' }).toString();
    return url.href;
  }

  /** Bound every request and preserve caller cancellation without leaking timeout or abort listeners. */
  async #request<T>(
    path: string,
    read: (response: Response) => Promise<T>,
    value?: object,
    signal?: AbortSignal,
  ): Promise<T> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    if (signal?.aborted) abort();
    const timeout = setTimeout(abort, 20000);
    try {
      const response = await this.fetch(path, {
        signal: controller.signal,
        ...(value
          ? {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(value),
            }
          : {}),
      });
      return await read(response);
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener('abort', abort);
    }
  }
}

/** Preserve structured server errors without relying on arbitrary response markup. */
async function responseError(response: Response, fallback: string): Promise<Error> {
  const value: unknown = await response.json();
  return new Error(
    isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
      ? value.error.message
      : fallback,
  );
}
