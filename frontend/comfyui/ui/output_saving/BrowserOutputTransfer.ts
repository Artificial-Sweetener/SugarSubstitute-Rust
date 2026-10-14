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
/** Materialize one deliberate transfer without retaining image data or duplicating format policy. */
import type { OutputFileReference } from '../execution/domain/WorkflowOutput.js';
import type { OutputImageClipboard } from './BrowserImageClipboard.js';
import {
  OutputTransferClient,
  outputTransferUrl,
  type OutputTransferSelection,
} from './OutputTransferClient.js';

/** Coordinate the shared file selection with browser-owned transport and clipboard boundaries. */
export class BrowserOutputTransfer {
  readonly #client: OutputTransferClient;

  /** Bind platform collaborators explicitly so native canvases can use the same selection endpoint. */
  constructor(
    private readonly origin: string,
    private readonly fetch: typeof globalThis.fetch,
    private readonly clipboard: OutputImageClipboard,
  ) {
    this.#client = new OutputTransferClient(fetch);
  }

  /** Copy exact preferred bytes only while the originating output remains current. */
  async copy(file: OutputFileReference, accepts: () => boolean): Promise<boolean> {
    const image = await this.#read(file, accepts);
    if (!image || !accepts()) return false;
    await this.clipboard.write(image.blob, image.selection.mimeType);
    return true;
  }

  /** Prepare exact preferred file bytes for a synchronous browser drag without retaining them here. */
  async drag(
    file: OutputFileReference,
    signal: AbortSignal,
  ): Promise<{ readonly file: File; readonly url: string } | null> {
    const image = await this.#read(file, () => !signal.aborted, signal);
    return image
      ? {
          file: new File([image.blob], image.selection.file.filename, {
            type: image.selection.mimeType,
          }),
          url: image.url,
        }
      : null;
  }

  /** Share bounded validation and byte transport between Copy and outbound dragging. */
  async #read(
    file: OutputFileReference,
    accepts: () => boolean,
    signal?: AbortSignal,
  ): Promise<{
    readonly blob: Blob;
    readonly selection: OutputTransferSelection;
    readonly url: string;
  } | null> {
    if (!accepts()) return null;
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    if (signal?.aborted) abort();
    const timer = setTimeout(() => controller.abort(), 20000);
    try {
      const selection = await this.#client.choose(file, controller.signal);
      if (!accepts() || controller.signal.aborted) return null;
      const url = outputTransferUrl(this.origin, selection);
      const response = await this.fetch(url, {
        signal: controller.signal,
      });
      if (!response.ok) throw new Error('The output image could not be read for transfer.');
      const blob = await response.blob();
      if (!accepts() || controller.signal.aborted) return null;
      if (blob.type !== selection.mimeType || !blob.size)
        throw new Error('The output transfer returned invalid image bytes.');
      return { blob, selection, url };
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
    }
  }
}
