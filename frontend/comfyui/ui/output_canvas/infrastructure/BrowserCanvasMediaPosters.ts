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
/** Cache bounded static media previews without retaining players, models, or decoded images. */
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';

export interface CanvasPosterRenderer {
  /** Capture one static preview and retire native resources before resolving. */
  render(item: WorkflowOutputItem, url: string, signal: AbortSignal): Promise<Blob>;
}

interface PosterCapture {
  readonly controller: AbortController;
  readonly completion: Promise<Blob>;
  readonly consumers: Set<symbol>;
}

/** Bound cached encoded posters independently of the canvas's decoded-pixel budget. */
export class BrowserCanvasMediaPosters {
  readonly #cache = new Map<string, Blob>();
  readonly #pending = new Map<string, PosterCapture>();
  #bytes = 0;
  #disposed = false;

  /** Inject the actual browser/host capture boundary. */
  constructor(private readonly renderer: CanvasPosterRenderer) {}

  /** Reuse an exact revision's poster and never cache a retired request. */
  async read(item: WorkflowOutputItem, url: string, signal: AbortSignal): Promise<Blob> {
    if (signal.aborted) throw signal.reason;
    if (this.#disposed) throw new Error('Media preview cache is disposed.');
    const key = JSON.stringify([
      item.provenance.sessionId,
      item.provenance.generation,
      item.provenance.runOrder,
      item.provenance.revision,
      item.file,
    ]);
    const cached = this.#cache.get(key);
    if (cached) {
      this.#cache.delete(key);
      this.#cache.set(key, cached);
      return cached;
    }
    let capture = this.#pending.get(key);
    if (!capture || capture.controller.signal.aborted) {
      const controller = new AbortController();
      const consumers = new Set<symbol>();
      const completion = Promise.resolve()
        .then(() => {
          if (controller.signal.aborted) throw controller.signal.reason;
          return this.renderer.render(item, url, controller.signal);
        })
        .then((blob) => {
          if (controller.signal.aborted) throw controller.signal.reason;
          this.#store(key, blob);
          return blob;
        })
        .finally(() => {
          if (this.#pending.get(key)?.controller === controller) this.#pending.delete(key);
        });
      capture = { controller, consumers, completion };
      this.#pending.set(key, capture);
    }
    return this.#subscribe(capture, signal);
  }

  /** Expose explicit encoded-preview accounting without recurring telemetry. */
  diagnostics(): { posters: number; posterBytes: number } {
    return { posters: this.#cache.size, posterBytes: this.#bytes };
  }
  /** Release encoded previews with the owning canvas host. */
  dispose(): void {
    this.#disposed = true;
    for (const capture of this.#pending.values()) capture.controller.abort();
    this.#pending.clear();
    this.#cache.clear();
    this.#bytes = 0;
  }

  /** Keep shared capture alive until its last visible/admitted consumer retires. */
  #subscribe(capture: PosterCapture, signal: AbortSignal): Promise<Blob> {
    const consumer = Symbol();
    capture.consumers.add(consumer);
    return new Promise((resolve, reject) => {
      let settled = false;
      const release = (): boolean => {
        if (settled) return false;
        settled = true;
        signal.removeEventListener('abort', aborted);
        capture.controller.signal.removeEventListener('abort', aborted);
        capture.consumers.delete(consumer);
        if (!capture.consumers.size) capture.controller.abort();
        return true;
      };
      const aborted = (): void => {
        if (release()) reject(new DOMException('Media preview request canceled.', 'AbortError'));
      };
      signal.addEventListener('abort', aborted, { once: true });
      capture.controller.signal.addEventListener('abort', aborted, { once: true });
      void capture.completion.then(
        (blob) => {
          if (release()) resolve(blob);
        },
        (error: unknown) => {
          if (release())
            reject(error instanceof Error ? error : new Error('Media preview capture failed.'));
        },
      );
    });
  }

  /** Retain encoded previews only within explicit count and byte limits. */
  #store(key: string, blob: Blob): void {
    if (this.#disposed || blob.size > 1024 * 1024) return;
    const previous = this.#cache.get(key);
    if (previous) this.#bytes -= previous.size;
    this.#cache.set(key, blob);
    this.#bytes += blob.size;
    while (this.#cache.size > 24 || this.#bytes > 8 * 1024 * 1024) {
      const oldest = this.#cache.entries().next().value;
      if (!oldest) break;
      this.#cache.delete(oldest[0]);
      this.#bytes -= oldest[1].size;
    }
  }
}
