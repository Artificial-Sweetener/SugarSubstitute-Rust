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
/** Coordinate one interpreted source preview with coherent, reusable full-composition context. */
import type { InputEditorBinding } from './InputEditorPorts.js';
import type { CanvasSourcePreviews, CanvasSourcePreview } from './CanvasSourcePreviews.js';
import type { InputEntry } from '../domain/InputBinding.js';
import { canvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import {
  canvasInputSourceKey,
  sameCanvasSourceDocument,
  type CanvasInputSource,
  type CanvasPixelRaster,
} from '../../core/embedded/CanvasRuntime.js';

/** Keep source interpretation separate from the presentation's single compositing pass. */
export type InputPreviewContentValue =
  | { readonly kind: 'loading' }
  | { readonly kind: 'error'; readonly message: string }
  | {
      readonly kind: 'pixels';
      readonly pixels: CanvasPixelRaster;
      readonly composition: CanvasPixelRaster | null;
    };

/** Own captures and asynchronous interpretation for one mounted source and display-resolution lifetime. */
export class InputPreviewContent {
  readonly #stops: (() => void)[] = [];
  readonly #hasComposition: boolean;
  #source: Extract<CanvasSourcePreview, { kind: 'pixels' }> | null = null;
  #composition: CanvasSourcePreview | null = null;
  #pixels: CanvasPixelRaster | null = null;
  #interpretation = '';
  #serial = 0;
  #alive = true;

  /** Invalidate source lifetimes only for capture identity, availability, file interpretation or display demand. */
  static reference(binding: InputEditorBinding, entry: InputEntry, maximumSize: number): string {
    const composition = compositionSource(binding, entry);
    return JSON.stringify([
      canvasInputSourceKey(entry.source),
      maximumSize,
      composition ? canvasInputSourceKey(composition) : null,
      entry.source.kind === 'canvas'
        ? canvasSourceUnavailable(entry.source, binding.context)
        : null,
      entry.source.kind === 'file' ? binding.preview.settings : null,
    ]);
  }

  /** Share subscriptions through the existing capture owner and keep native full-resolution custom interpretation bounded at display. */
  constructor(
    private readonly binding: InputEditorBinding,
    previews: CanvasSourcePreviews,
    entry: InputEntry,
    private readonly maximumSize: number,
    private readonly publish: (value: InputPreviewContentValue) => void,
    private readonly report: (error: unknown) => void,
  ) {
    const composition = compositionSource(binding, entry);
    this.#hasComposition = composition !== null;
    const reason =
      entry.source.kind === 'canvas'
        ? canvasSourceUnavailable(entry.source, binding.context)
        : null;
    if (reason || (entry.source.kind === 'file' && !entry.source.file)) {
      publish({ kind: 'error', message: reason ?? 'Choose a file' });
      return;
    }
    if (composition)
      this.#stops.push(
        previews.observe(
          composition,
          (value) => {
            this.#composition = value;
            this.#publishPixels();
          },
          maximumSize,
          'image',
        ),
      );
    if (entry.source.kind === 'canvas') {
      this.#stops.push(
        previews.observe(
          entry.source,
          (value) => {
            this.#source = value.kind === 'pixels' ? value : null;
            this.#pixels = null;
            this.#interpretation = '';
            this.#serial++;
            if (value.kind === 'pixels') this.refresh();
            else
              publish(
                value.kind === 'loading'
                  ? value
                  : {
                      kind: 'error',
                      message: value.unavailable ? value.message : 'Preview unavailable',
                    },
              );
          },
          binding.preview.captureSize(maximumSize),
          binding.loader.kind === 'image' ? 'image' : 'raw',
        ),
      );
    } else if (entry.source.file) {
      publish({ kind: 'loading' });
      void binding.preview
        .file(entry.source.file, maximumSize)
        .then((pixels) => {
          if (!this.#alive) return;
          this.#pixels = pixels;
          this.#publishPixels();
        })
        .catch((error: unknown) => this.#failed(error));
    }
  }

  /** Reinterpret settings from cached raw pixels without another native source or composition capture. */
  refresh(): void {
    const source = this.#source;
    if (!this.#alive || !source) return;
    const settings = JSON.stringify(this.binding.preview.settings);
    if (this.#interpretation === settings) return;
    this.#interpretation = settings;
    const serial = ++this.#serial;
    this.publish({ kind: 'loading' });
    void this.binding.preview
      .canvas(source.pixels, this.maximumSize)
      .then((pixels) => {
        if (!this.#alive || this.#source !== source || this.#serial !== serial) return;
        this.#pixels = pixels;
        this.#publishPixels();
      })
      .catch((error: unknown) => {
        if (this.#alive && this.#source === source && this.#serial === serial) this.#failed(error);
      });
  }

  /** Retire pending interpretations before releasing every shared native subscription. */
  dispose(): void {
    this.#alive = false;
    for (const stop of this.#stops) stop();
    this.#stops.length = 0;
  }

  /** Keep mismatched native revisions on the last displayed frame until both captures catch up. */
  #publishPixels(): void {
    if (!this.#alive || !this.#pixels) return;
    let composition: CanvasPixelRaster | null = null;
    if (this.#hasComposition) {
      const value = this.#composition;
      if (value?.kind === 'error') {
        this.publish({
          kind: 'error',
          message: value.unavailable ? value.message : 'Preview unavailable',
        });
        return;
      }
      if (
        value?.kind !== 'pixels' ||
        (this.#source &&
          (value.revision !== this.#source.revision ||
            !sameCanvasSourceDocument(value.document, this.#source.document)))
      ) {
        this.publish({ kind: 'loading' });
        return;
      }
      composition = value.pixels;
    }
    this.publish({ kind: 'pixels', pixels: this.#pixels, composition });
  }

  /** Report unexpected codec failures while leaving concise feedback inside the persistent preview. */
  #failed(error: unknown): void {
    if (!this.#alive) return;
    this.publish({ kind: 'error', message: 'Preview unavailable' });
    this.report(error);
  }
}

/** Request plain composition pixels only from a connected canvas that advertises this capability. */
function compositionSource(
  binding: InputEditorBinding,
  entry: InputEntry,
): Exclude<CanvasInputSource, { kind: 'file' }> | null {
  const context = binding.context;
  if (
    binding.loader.kind !== 'mask' ||
    !context?.supported ||
    !context.document ||
    !context.targets.includes('composition')
  )
    return null;
  return {
    kind: 'canvas',
    canvasId: entry.source.kind === 'canvas' ? entry.source.canvasId : binding.canvasId,
    documentId: entry.source.kind === 'canvas' ? entry.source.documentId : context.document.id,
    target: { kind: 'composition' },
  };
}
