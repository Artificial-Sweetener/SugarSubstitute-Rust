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
/** Own browser-only canvas resources for documents that select browser presentation. */

import { isRecord } from '../../types/common.js';
import { CanvasImageAllowances } from '../application/CanvasImageAllowances.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import { BrowserCanvasAttachment } from './BrowserCanvasAttachment.js';
import { BrowserCanvasImageLoader } from './BrowserCanvasImageLoader.js';
import { BrowserCanvasDecodedImages } from './BrowserCanvasDecodedImages.js';
import type { MemoryBudgetRegistry } from '../../core/memory/MemoryBudgetRegistry.js';
import { BrowserCanvasMediaPosters } from './BrowserCanvasMediaPosters.js';
import { BrowserCanvasMediaSessions } from './BrowserCanvasMediaSessions.js';
import { BrowserCanvasPosterRenderer } from './BrowserCanvasPosterRenderer.js';
import type { CanvasAttachmentOptions } from './CanvasAttachmentOptions.js';
import { comfyCanvasMediaUrl, openComfyCanvasMedia } from './ComfyCanvasMedia.js';
import { ComfyCanvasModelRuntime } from './ComfyCanvasModelRuntime.js';

/** Retain browser presentation caches independently of shared workflow and controls. */
export class ComfyBrowserCanvasHost {
  readonly #allowances = new CanvasImageAllowances();
  readonly #sessions = new BrowserCanvasMediaSessions();
  readonly #models: ComfyCanvasModelRuntime;
  readonly #posters: BrowserCanvasMediaPosters;
  readonly #images: BrowserCanvasDecodedImages;
  readonly #window: Window & typeof globalThis;

  /** Retain posters and decoded images for the page lifetime under its memory allowance. */
  constructor(
    private readonly document: Document,
    private readonly logger: Pick<Console, 'error'>,
    private readonly memory: MemoryBudgetRegistry,
  ) {
    const window = document.defaultView;
    if (!window) throw new Error('Output canvas window is unavailable.');
    this.#window = window;
    this.#models = new ComfyCanvasModelRuntime(() => (isRecord(window) ? window.comfyAPI : null));
    this.#posters = new BrowserCanvasMediaPosters(
      new BrowserCanvasPosterRenderer(document, this.#models),
      memory,
    );
    this.#images = new BrowserCanvasDecodedImages(
      new BrowserCanvasImageLoader(
        document,
        {
          create: (blob) => window.URL.createObjectURL(blob),
          revoke: (url) => window.URL.revokeObjectURL(url),
        },
        this.#posters,
      ),
      memory,
    );
  }

  /** Mount a browser presenter only after the view's preparation is still current. */
  mount(element: HTMLElement, shared: CanvasAttachmentOptions): BrowserCanvasAttachment {
    const loader = this.#images.attachment();
    return new BrowserCanvasAttachment(element, {
      ...shared,
      allowances: this.#allowances,
      loader,
      memory: this.memory,
      logger: this.logger,
      models: this.#models,
      mediaSessions: this.#sessions,
      url: (item) => comfyCanvasMediaUrl(this.document.location.origin, item),
      open: (item) => openComfyCanvasMedia(this.#window, item),
      disposeLoader: () => loader.dispose(),
    });
  }

  /** Retire allowance state when its workflow view leaves the shared registry. */
  remove(view: CanvasViewId): void {
    this.#allowances.remove(view);
  }

  /** Report cached browser posters and parked images only at an explicit diagnostics boundary. */
  diagnostics(): {
    posters: number;
    posterBytes: number;
    parkedImages: number;
    parkedBytes: number;
  } {
    return { ...this.#posters.diagnostics(), ...this.#images.diagnostics() };
  }

  /** Release browser-only session, image and poster resources with the owning document. */
  dispose(): void {
    this.#images.dispose();
    this.#posters.dispose();
    this.#sessions.dispose();
  }
}
