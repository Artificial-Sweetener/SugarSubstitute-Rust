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
/** Decode ordinary browser images while owning only this surface's elements and preview URLs. */

import {
  canvasImageBytes,
  type CanvasDecodedImage,
  type CanvasImageInput,
  type CanvasImageLoad,
  type CanvasImageLoadPort,
} from '../domain/CanvasImageResource.js';

export type BrowserCanvasImagePayload =
  | { readonly kind: 'file'; readonly url: string }
  | { readonly kind: 'preview'; readonly blob: Blob };

export interface BrowserCanvasImageUrls {
  /** Allocate a URL only after shared resource admission. */
  create(blob: Blob): string;
  /** Revoke only URLs allocated by this loader. */
  revoke(url: string): void;
}

/** Keep platform decode and URL release behind the generic image-loading port. */
export class BrowserCanvasImageLoader
  implements CanvasImageLoadPort<BrowserCanvasImagePayload, HTMLImageElement>
{
  readonly #releases = new Set<() => void>();
  readonly #urls = new Set<string>();
  #disposed = false;

  /** Inject the attachment document and URL allocator for explicit window ownership. */
  constructor(
    private readonly document: Document,
    private readonly urls: BrowserCanvasImageUrls,
  ) {}

  /** Start one admitted ordinary image without copying pixels or retaining preview bytes. */
  start(input: CanvasImageInput<BrowserCanvasImagePayload>): CanvasImageLoad<HTMLImageElement> {
    if (this.#disposed) throw new Error('Canvas image loader is disposed.');
    const image = this.document.createElement('img');
    image.decoding = 'async';
    image.draggable = false;
    image.alt = '';
    let url: string | null = null;
    let settled = false;
    let released = false;
    let ready: (value: CanvasDecodedImage<HTMLImageElement>) => void = () => {};
    let failed: (error: unknown) => void = () => {};
    const completion = new Promise<CanvasDecodedImage<HTMLImageElement>>((resolve, reject) => {
      ready = resolve;
      failed = reject;
    });
    /** Relinquish this allocation once, including a platform decode that settles after cancellation. */
    const release = (): void => {
      if (released) return;
      released = true;
      image.removeEventListener('load', loaded);
      image.removeEventListener('error', errored);
      image.removeAttribute('src');
      image.remove();
      this.#releases.delete(cancel);
      if (url !== null) {
        this.#urls.delete(url);
        this.urls.revoke(url);
        url = null;
      }
    };
    /** Reject a failed/canceled load while retaining its original diagnostic exception. */
    const reject = (error: unknown): void => {
      if (settled) return;
      settled = true;
      release();
      failed(error);
    };
    /** Accept valid intrinsic geometry only while this exact allocation remains owned. */
    const accept = (): void => {
      if (settled || released) return;
      try {
        const dimensions = Object.freeze({
          width: image.naturalWidth,
          height: image.naturalHeight,
        });
        canvasImageBytes(dimensions);
        settled = true;
        ready(Object.freeze({ handle: image, dimensions, release }));
      } catch (error: unknown) {
        reject(error);
      }
    };
    /** Support hosts without decode while preferring decode completion when available. */
    const loaded = (): void => {
      if (typeof image.decode !== 'function') accept();
    };
    /** Translate browser load failure to a recoverable platform error. */
    const errored = (): void => {
      reject(new Error('Browser image loading failed.'));
    };
    /** End both promise delivery and platform ownership at this surface's lifetime boundary. */
    const cancel = (): void => {
      reject(new Error('Canvas image loading canceled.'));
      release();
    };
    this.#releases.add(cancel);
    image.addEventListener('load', loaded);
    image.addEventListener('error', errored);
    try {
      if (input.payload.kind === 'preview') {
        url = this.urls.create(input.payload.blob);
        this.#urls.add(url);
        image.src = url;
      } else {
        const target = new URL(input.payload.url, this.document.baseURI);
        const origin = new URL(this.document.baseURI).origin;
        if (!['http:', 'https:'].includes(target.protocol) || target.origin !== origin)
          throw new Error('Canvas file URL must use the current host origin.');
        image.src = target.href;
      }
      if (typeof image.decode === 'function') void image.decode().then(accept, reject);
    } catch (error: unknown) {
      reject(error);
      // A start failure has no returned promise consumer; contain its rejection before rethrowing.
      void completion.catch(() => {});
      throw error;
    }
    return {
      completion,
      cancel,
    };
  }

  /** Count only image elements and URLs explicitly owned by this surface. */
  diagnostics(): { readonly elements: number; readonly previewUrls: number } {
    return Object.freeze({ elements: this.#releases.size, previewUrls: this.#urls.size });
  }

  /** Release every platform allocation at the attachment/window lifetime boundary. */
  dispose(): void {
    this.#disposed = true;
    for (const release of [...this.#releases]) release();
  }
}
