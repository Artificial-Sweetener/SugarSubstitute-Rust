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
/** Resolve one loader's actual selected source through shared capture, codec and interpretation owners. */
import type { InputEditorBinding } from './InputEditorPorts.js';
import type { CanvasSourcePreviews, CanvasSourcePreview } from './CanvasSourcePreviews.js';
import type { InputEntry } from '../domain/InputBinding.js';
import { canvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import {
  canvasInputSourceKey,
  type CanvasPixelRaster,
  type CanvasDocument,
} from '../../core/embedded/CanvasRuntime.js';

/** Retain native revision identity for consumers combining related sources. */
export type InputSourcePreviewValue =
  | { readonly kind: 'loading' }
  | { readonly kind: 'error'; readonly message: string }
  | {
      readonly kind: 'pixels';
      readonly pixels: CanvasPixelRaster;
      readonly canvas: {
        readonly id: string;
        readonly document: CanvasDocument;
        readonly revision: string;
      } | null;
    };

/** Own asynchronous source work independently of preview geometry and relationships. */
export class InputSourcePreview {
  readonly #stop: () => void;
  #source: Extract<CanvasSourcePreview, { kind: 'pixels' }> | null = null;
  #interpretation = '';
  #serial = 0;
  #alive = true;

  /** Keep availability policy shared by source admission and lifetime invalidation. */
  static unavailable(binding: InputEditorBinding, entry: InputEntry): string | null {
    return entry.source.kind === 'canvas'
      ? canvasSourceUnavailable(entry.source, binding.context)
      : entry.source.file
        ? null
        : 'Choose a file';
  }
  /** Reuse source work across interpretation edits while file codecs keep their authored settings. */
  static reference(binding: InputEditorBinding, entry: InputEntry, maximumSize: number): string {
    return JSON.stringify([
      canvasInputSourceKey(entry.source),
      maximumSize,
      this.unavailable(binding, entry),
      entry.source.kind === 'file' ? binding.preview.settings : null,
    ]);
  }

  /** Read canvas or file data through the same source path used by the loader's own preview. */
  constructor(
    private readonly binding: InputEditorBinding,
    previews: CanvasSourcePreviews,
    private readonly entry: InputEntry,
    private readonly maximumSize: number,
    private readonly publish: (value: InputSourcePreviewValue) => void,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stop = () => {};
    const reason = InputSourcePreview.unavailable(binding, entry);
    if (reason) {
      publish({ kind: 'error', message: reason });
      return;
    }
    if (entry.source.kind === 'canvas') {
      this.#stop = previews.observe(
        entry.source,
        (value) => {
          this.#source = value.kind === 'pixels' ? value : null;
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
      );
    } else if (entry.source.file) {
      publish({ kind: 'loading' });
      void binding.preview
        .file(entry.source.file, maximumSize)
        .then((pixels) => {
          if (this.#alive) publish({ kind: 'pixels', pixels, canvas: null });
        })
        .catch((error: unknown) => this.#failed(error));
    }
  }

  /** Reinterpret the current native snapshot without repeating capture or file staging. */
  refresh(): void {
    const source = this.#source;
    if (!this.#alive || !source || this.entry.source.kind !== 'canvas') return;
    const settings = JSON.stringify(this.binding.preview.settings);
    if (this.#interpretation === settings) return;
    this.#interpretation = settings;
    const serial = ++this.#serial,
      canvasId = this.entry.source.canvasId;
    this.publish({ kind: 'loading' });
    void this.binding.preview
      .canvas(source.pixels, this.maximumSize)
      .then((pixels) => {
        if (this.#alive && this.#source === source && this.#serial === serial)
          this.publish({
            kind: 'pixels',
            pixels,
            canvas: { id: canvasId, document: source.document, revision: source.revision },
          });
      })
      .catch((error: unknown) => {
        if (this.#alive && this.#source === source && this.#serial === serial) this.#failed(error);
      });
  }

  /** Retire pending results before releasing the shared source subscription. */
  dispose(): void {
    this.#alive = false;
    this.#stop();
  }

  /** Preserve codec failure context while publishing concise source feedback. */
  #failed(error: unknown): void {
    if (!this.#alive) return;
    this.publish({ kind: 'error', message: 'Preview unavailable' });
    this.report(error);
  }
}
