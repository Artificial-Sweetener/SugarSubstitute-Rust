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
/** Capture finite static posters using native media decoding and Comfy's thumbnail renderer. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasModelRuntime } from './ComfyCanvasModelRuntime.js';
import type { CanvasPosterRenderer } from './BrowserCanvasMediaPosters.js';

/** Retire every temporary capture surface before handing bounded bytes to the cache owner. */
export class BrowserCanvasPosterRenderer implements CanvasPosterRenderer {
  /** Bind the attachment document and the existing Comfy model runtime. */
  constructor(
    private readonly document: Document,
    private readonly models: CanvasModelRuntime,
  ) {}

  /** Produce one nonanimated preview without starting ongoing playback. */
  async render(item: WorkflowOutputItem, url: string, signal: AbortSignal): Promise<Blob> {
    if (signal.aborted) throw signal.reason;
    if (item.mediaKind === 'model') return this.#model(item, url, signal);
    return this.#pixels(item.mediaKind === 'video', url, signal);
  }

  /** Use Comfy's own representative model camera and release its temporary WebGL context. */
  async #model(item: WorkflowOutputItem, url: string, signal: AbortSignal): Promise<Blob> {
    const container = this.document.createElement('div');
    container.className = 'sc-output-poster-capture';
    container.setAttribute('aria-hidden', 'true');
    this.document.body.append(container);
    try {
      const viewer = await this.models.load(container, url, item.file.filename, signal);
      try {
        const encoded = await viewer.capture();
        if (signal.aborted) throw signal.reason;
        const split = encoded.indexOf(',');
        const bytes = Uint8Array.from(atob(encoded.slice(split + 1)), (character) =>
          character.charCodeAt(0),
        );
        return new Blob([bytes], { type: encoded.slice(5, encoded.indexOf(';')) });
      } finally {
        viewer.dispose();
      }
    } finally {
      container.remove();
    }
  }

  /** Read a first decoded frame with a bounded deadline, then release the source decoder. */
  #pixels(video: boolean, url: string, signal: AbortSignal): Promise<Blob> {
    return new Promise<Blob>((resolve, reject) => {
      const media = video
        ? this.document.createElement('video')
        : this.document.createElement('img');
      const cleanup = (): void => {
        clearTimeout(timer);
        signal.removeEventListener('abort', aborted);
        media.removeEventListener(video ? 'loadeddata' : 'load', loaded);
        media.removeEventListener('error', failed);
        media.removeAttribute('src');
        if (media instanceof HTMLVideoElement) {
          media.pause();
          media.load();
        }
      };
      const fail = (error: unknown): void => {
        cleanup();
        reject(error instanceof Error ? error : new Error('Media preview capture was canceled.'));
      };
      const aborted = (): void => fail(signal.reason);
      const failed = (): void =>
        fail(new Error('The browser could not decode this media preview.'));
      const loaded = (): void => {
        try {
          if (signal.aborted) throw signal.reason;
          const width = media instanceof HTMLVideoElement ? media.videoWidth : media.naturalWidth;
          const height =
            media instanceof HTMLVideoElement ? media.videoHeight : media.naturalHeight;
          if (!width || !height) throw new Error('Media preview dimensions are unavailable.');
          const scale = Math.min(1, 512 / Math.max(width, height));
          const canvas = this.document.createElement('canvas');
          canvas.width = Math.max(1, Math.round(width * scale));
          canvas.height = Math.max(1, Math.round(height * scale));
          const context = canvas.getContext('2d');
          if (!context) throw new Error('Media preview capture is unavailable.');
          context.drawImage(media, 0, 0, canvas.width, canvas.height);
          cleanup();
          canvas.toBlob(
            (blob) => (blob ? resolve(blob) : reject(new Error('Media preview encoding failed.'))),
            'image/webp',
            0.8,
          );
        } catch (error: unknown) {
          fail(error);
        }
      };
      const timer = setTimeout(() => fail(new Error('Media preview loading timed out.')), 8000);
      media.addEventListener(video ? 'loadeddata' : 'load', loaded, { once: true });
      media.addEventListener('error', failed, { once: true });
      signal.addEventListener('abort', aborted, { once: true });
      if (media instanceof HTMLVideoElement) {
        media.preload = 'auto';
        media.muted = true;
        media.playsInline = true;
      }
      media.src = url;
    });
  }
}
