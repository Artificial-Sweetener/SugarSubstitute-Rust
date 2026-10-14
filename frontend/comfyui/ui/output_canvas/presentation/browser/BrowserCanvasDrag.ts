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
/** Prepare one hovered completed image for the browser's synchronous outbound drag boundary. */
export interface BrowserCanvasDragSubject {
  readonly key: string;
  readonly element: HTMLImageElement;
}

export interface BrowserCanvasDragPayload {
  readonly file: File;
  readonly url: string;
}

export interface BrowserCanvasDragOptions {
  /** Resolve only current, displayed, draggable image pixels. */
  resolve(target: EventTarget | null, event: MouseEvent | null): BrowserCanvasDragSubject | null;
  /** Apply shared transfer policy before the browser makes its data store read-only. */
  prepare(key: string, signal: AbortSignal): Promise<BrowserCanvasDragPayload | null>;
  /** Fence the fixed attachment and its visibility. */
  accepts(): boolean;
  /** Report current failures through product feedback. */
  report(error: unknown): void;
}

interface DragPreparation {
  readonly subject: BrowserCanvasDragSubject;
  readonly controller: AbortController;
  event: MouseEvent;
  payload: BrowserCanvasDragPayload | null;
}

/** Retain at most one encoded image and retire it on navigation, hiding, leaving, or disposal. */
export class BrowserCanvasDrag {
  #preparation: DragPreparation | null = null;
  #disposed = false;

  /** Delegate only local image gestures without observing host DOM or owning image decoding. */
  constructor(
    private readonly root: HTMLElement,
    private readonly options: BrowserCanvasDragOptions,
  ) {
    root.addEventListener('pointerover', this.#hover);
    root.addEventListener('pointermove', this.#hover);
    root.addEventListener('pointerleave', this.#leave);
    root.addEventListener('dragstart', this.#start);
    root.addEventListener('dragend', this.#leave);
  }

  /** Let fitted mouse presses use native dragging while touch and enlarged images retain navigation. */
  allows(event: MouseEvent): boolean {
    return !this.#disposed && this.options.accepts() && !!this.options.resolve(event.target, event);
  }

  /** Invalidate bytes when their displayed identity or fitted footprint changes. */
  reconcile(): void {
    const prepared = this.#preparation;
    if (!prepared) return;
    const current = this.options.accepts()
      ? this.options.resolve(prepared.subject.element, prepared.event)
      : null;
    if (current?.key !== prepared.subject.key || current.element !== prepared.subject.element)
      this.#clear();
  }

  /** Discard a prepared representation when shared transfer preferences change. */
  invalidate(): void {
    this.#clear();
  }

  /** Cancel finite preparation and unregister exactly this surface's listeners. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#clear();
    this.root.removeEventListener('pointerover', this.#hover);
    this.root.removeEventListener('pointermove', this.#hover);
    this.root.removeEventListener('pointerleave', this.#leave);
    this.root.removeEventListener('dragstart', this.#start);
    this.root.removeEventListener('dragend', this.#leave);
  }

  /** Prepare only the pointer's current exact image, never a whole workflow or batch. */
  readonly #hover = (event: PointerEvent): void => {
    if (this.#disposed || event.pointerType === 'touch') return;
    const subject = this.options.accepts() ? this.options.resolve(event.target, event) : null;
    if (
      subject?.key === this.#preparation?.subject.key &&
      subject?.element === this.#preparation?.subject.element
    ) {
      if (this.#preparation) this.#preparation.event = event;
      return;
    }
    this.#clear();
    if (!subject) return;
    const preparation: DragPreparation = {
      subject,
      controller: new AbortController(),
      event,
      payload: null,
    };
    this.#preparation = preparation;
    subject.element.draggable = true;
    void this.#prepare(preparation);
  };

  /** Fence asynchronous success and failure to the single current image lease. */
  async #prepare(preparation: DragPreparation): Promise<void> {
    try {
      const payload = await this.options.prepare(
        preparation.subject.key,
        preparation.controller.signal,
      );
      this.reconcile();
      if (this.#preparation === preparation && !preparation.controller.signal.aborted)
        preparation.payload = payload;
    } catch (error) {
      this.reconcile();
      if (this.#preparation !== preparation || preparation.controller.signal.aborted) return;
      preparation.controller.abort();
      preparation.subject.element.draggable = false;
      this.options.report(error);
    }
  }

  /** Fill the browser-owned drag store synchronously with exact preferred bytes and a download URL. */
  readonly #start = (event: DragEvent): void => {
    this.reconcile();
    const preparation = this.#preparation;
    const subject = this.options.accepts() ? this.options.resolve(event.target, event) : null;
    const payload = preparation?.payload;
    if (
      !payload ||
      !event.dataTransfer ||
      subject?.key !== preparation?.subject.key ||
      subject?.element !== preparation?.subject.element
    ) {
      event.preventDefault();
      return;
    }
    event.stopPropagation();
    try {
      // Chromium supplies an image named "view" before dragstart; retain only the selected representation.
      event.dataTransfer.items.clear();
      event.dataTransfer.effectAllowed = 'copy';
      event.dataTransfer.items.add(payload.file);
      event.dataTransfer.setData(
        'DownloadURL',
        `${payload.file.type}:${payload.file.name}:${payload.url}`,
      );
      event.dataTransfer.setData('text/uri-list', payload.url);
    } catch (error) {
      event.preventDefault();
      this.#clear();
      this.options.report(error);
    }
  };

  /** Release retained bytes without releasing the resource owner's decoded image. */
  #clear(): void {
    const preparation = this.#preparation;
    this.#preparation = null;
    if (!preparation) return;
    preparation.controller.abort();
    preparation.subject.element.draggable = false;
    preparation.payload = null;
  }

  /** End hover or completed native dragging without scheduling idle work. */
  readonly #leave = (): void => this.#clear();
}
