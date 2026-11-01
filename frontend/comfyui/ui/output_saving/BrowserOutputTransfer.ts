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
import type { OutputImageDownload } from './BrowserImageDownload.js';
import type { OutputCopyFormat, OutputMediaProfile } from './domain/OutputMedia.js';
import { OutputMediaClient } from './OutputMediaClient.js';
import { OutputMediaProfileSource } from './OutputMediaProfileSource.js';
import {
  OutputTransferClient,
  outputFileUrl,
  type OutputTransferSelection,
} from './OutputTransferClient.js';

/** Whether a file action may use still-image renditions or must keep the exact media file. */
export type OutputFileContent = 'image' | 'media';

/** Coordinate the shared file selection with browser-owned transport and clipboard boundaries. */
export class BrowserOutputTransfer {
  readonly #client: OutputTransferClient;
  readonly #media: OutputMediaClient;
  readonly #profile: OutputMediaProfileSource;
  #disposed = false;

  /** Bind platform collaborators explicitly so native canvases can use the same selection endpoint. */
  constructor(
    private readonly origin: string,
    private readonly fetch: typeof globalThis.fetch,
    private readonly clipboard: OutputImageClipboard,
    private readonly download: OutputImageDownload,
  ) {
    this.#client = new OutputTransferClient(fetch);
    this.#media = new OutputMediaClient(origin, fetch);
    this.#profile = new OutputMediaProfileSource(() => this.#media.profile());
  }

  /** Expose a conservative cached profile without network work when a context menu opens. */
  profile(): OutputMediaProfile {
    return this.#profile.read();
  }

  /** Observe accepted profile changes independently of canvas or image resource identity. */
  subscribe(listener: () => void): () => void {
    return this.#profile.subscribe(listener);
  }

  /** Refresh only on setup or accepted preference changes, refusing superseded responses. */
  refresh(): Promise<void> {
    return this.#profile.refresh();
  }

  /** Copy exact preferred bytes only while the originating output remains current. */
  async copy(
    file: OutputFileReference,
    accepts: () => boolean,
    format: OutputCopyFormat,
    signal?: AbortSignal,
  ): Promise<boolean> {
    if (this.#disposed || signal?.aborted || !accepts()) return false;
    const image = await this.#media.image(file, format, signal);
    if (this.#disposed || signal?.aborted || !accepts()) return false;
    await this.clipboard.write(image.blob, image.mimeType);
    return true;
  }

  /**
   * Reveal through the local backend or start the browser download without rendering another image.
   *
   * Only still images have preferred renditions, so other media download their exact file.
   */
  async fileAction(
    file: OutputFileReference,
    accepts: () => boolean,
    signal?: AbortSignal,
    content: OutputFileContent = 'image',
  ): Promise<boolean> {
    if (this.#disposed || signal?.aborted || !accepts()) return false;
    if (this.#profile.read().fileAction === 'reveal') await this.#media.reveal(file, signal);
    else
      this.download.start(
        content === 'image' ? this.#media.downloadUrl(file) : outputFileUrl(this.origin, file),
      );
    return true;
  }

  /** Retire profile listeners and refuse late clipboard writes after the browser host is torn down. */
  dispose(): void {
    this.#disposed = true;
    this.#profile.dispose();
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
    if (this.#disposed || !accepts()) return null;
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    if (signal?.aborted) abort();
    const timer = setTimeout(() => controller.abort(), 20000);
    try {
      const selection = await this.#client.choose(file, controller.signal);
      if (this.#disposed || !accepts() || controller.signal.aborted) return null;
      const url = outputFileUrl(this.origin, selection.file);
      const response = await this.fetch(url, {
        signal: controller.signal,
      });
      if (!response.ok) throw new Error('The output image could not be read for transfer.');
      const blob = await response.blob();
      if (this.#disposed || !accepts() || controller.signal.aborted) return null;
      if (blob.type !== selection.mimeType || !blob.size)
        throw new Error('The output transfer returned invalid image bytes.');
      return { blob, selection, url };
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
    }
  }
}
