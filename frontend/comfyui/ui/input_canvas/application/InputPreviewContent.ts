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
/** Combine loader previews with their graph-associated image source without inventing a canvas background. */
import type { InputEditorBinding, InputImageAssociation } from './InputEditorPorts.js';
import type { CanvasSourcePreviews } from './CanvasSourcePreviews.js';
import { InputSourcePreview, type InputSourcePreviewValue } from './InputSourcePreview.js';
import type { InputEntry } from '../domain/InputBinding.js';
import { DEFAULT_MASK_EDITS, hasMaskEdits } from '../domain/MaskEditSettings.js';
import type { MaskPreviewMode, MaskPreviewPlacement } from '../domain/MaskPreviewGeometry.js';
import { InputMaskPreview } from './InputMaskPreview.js';
import {
  sameCanvasSourceDocument,
  type CanvasPixelRaster,
} from '../../core/embedded/CanvasRuntime.js';

/** Keep interpreted content, its actual image context and empty-image presentation explicit. */
export type InputPreviewContentValue =
  | { readonly kind: 'loading' }
  | { readonly kind: 'black' }
  | { readonly kind: 'error'; readonly message: string }
  | {
      readonly kind: 'pixels';
      readonly pixels: CanvasPixelRaster;
      readonly background: CanvasPixelRaster | null;
      readonly frame: { readonly width: number; readonly height: number } | null;
      readonly placement?: MaskPreviewPlacement;
      readonly outline?: boolean;
      readonly viewport?: MaskPreviewPlacement;
    };

/** Consume reusable associations while source acquisition stays with the shared loader preview owner. */
export class InputPreviewContent {
  readonly #stopAssociation: () => void;
  #stopImageBinding: () => void = () => {};
  #imageBinding: InputEditorBinding | null = null;
  #sourceReader: InputSourcePreview | null = null;
  #imageReader: InputSourcePreview | null = null;
  #source: InputSourcePreviewValue | null = null;
  #image: InputSourcePreviewValue | null = null;
  #imageReference = '';
  #alive = true;
  #sourceSize = -1;
  readonly #edited: InputMaskPreview;

  /** Keep the primary source lifetime independent of relationship and associated image value changes. */
  static reference(binding: InputEditorBinding, entry: InputEntry, maximumSize: number): string {
    return InputSourcePreview.reference(binding, entry, maximumSize);
  }

  /** Observe graph relationships and exact image binding changes without graph discovery from presentation. */
  constructor(
    private readonly binding: InputEditorBinding,
    private readonly previews: CanvasSourcePreviews,
    private readonly entry: InputEntry,
    private readonly maximumSize: number,
    private readonly publish: (value: InputPreviewContentValue) => void,
    private readonly report: (error: unknown) => void,
    private readonly association: InputImageAssociation | null,
    private readonly mode: () => MaskPreviewMode,
  ) {
    this.#edited = new InputMaskPreview(binding.maskEditing, publish, report);
    this.#stopAssociation =
      binding.loader.kind === 'mask'
        ? (association?.subscribe(() => this.#refreshImage()) ?? (() => {}))
        : () => {};
    if (binding.loader.kind === 'mask') this.#refreshImage();
    else this.#startSource();
  }

  /** Apply settings changes through existing source work while related source choices retain their own owner. */
  refresh(): void {
    if (!this.#alive) return;
    if (this.binding.loader.kind === 'mask') this.#refreshImage();
    else this.#sourceReader?.refresh();
  }

  /** Preserve black for missing image context while valid image previews retain concise source feedback. */
  unavailable(message: string): void {
    this.#edited.invalidate();
    this.publish(
      this.binding.loader.kind === 'mask' && this.#image?.kind !== 'pixels'
        ? { kind: 'black' }
        : { kind: 'error', message },
    );
  }

  /** Release relationships and source observers before accepting any late source results. */
  dispose(): void {
    this.#alive = false;
    this.#stopAssociation();
    this.#stopImageBinding();
    this.#sourceReader?.dispose();
    this.#imageReader?.dispose();
    this.#edited.dispose();
  }

  /** Follow the associated loader's selected entry and actual source availability. */
  #refreshImage(): void {
    if (!this.#alive) return;
    try {
      const image = this.association?.read() ?? null;
      if (image !== this.#imageBinding) {
        this.#stopImageBinding();
        this.#imageBinding = image;
        this.#stopImageBinding = image?.subscribe(() => this.#refreshImage()) ?? (() => {});
        this.#imageReference = '';
      }
      const entry = image?.state.entries.find((entry) => entry.id === image.state.selected);
      if (
        !image ||
        image.loader.kind !== 'image' ||
        image.busy ||
        image.error ||
        !entry ||
        InputSourcePreview.unavailable(image, entry)
      ) {
        this.#black();
        return;
      }
      const imageSize = this.maximumSize;
      const reference = JSON.stringify([
        entry.id,
        InputSourcePreview.reference(image, entry, imageSize),
      ]);
      if (this.#imageReference !== reference) {
        this.#edited.invalidate();
        this.#imageReference = reference;
        this.#image = null;
        const previous = this.#imageReader;
        this.publish({ kind: 'loading' });
        if (!this.#sourceReader || this.#sourceSize !== this.#captureSize()) this.#startSource();
        this.#imageReader = new InputSourcePreview(
          image,
          this.previews,
          entry,
          imageSize,
          (value) => {
            this.#image = value;
            this.#publishPixels();
          },
          this.report,
        );
        previous?.dispose();
      } else this.#imageReader?.refresh();
      if (this.#sourceSize !== this.#captureSize()) this.#startSource();
      this.#sourceReader?.refresh();
      this.#publishPixels();
    } catch (error: unknown) {
      this.#black();
      this.report(error);
    }
  }

  /** Reuse the primary loader's source path for both ordinary image previews and effective mask pixels. */
  #startSource(): void {
    this.#edited.invalidate();
    this.#sourceReader?.dispose();
    this.#source = null;
    this.#sourceSize = this.#captureSize();
    this.#sourceReader = new InputSourcePreview(
      this.binding,
      this.previews,
      this.entry,
      this.#sourceSize,
      (value) => {
        this.#source = value;
        this.#publishPixels();
      },
      this.report,
    );
  }

  /** Release unnecessary capture work when no real image is selected and keep the preview solid black. */
  #black(): void {
    this.#edited.invalidate();
    this.#sourceReader?.dispose();
    this.#imageReader?.dispose();
    this.#sourceReader = this.#imageReader = null;
    this.#source = this.#image = null;
    this.#imageReference = '';
    this.#sourceSize = -1;
    this.publish({ kind: 'black' });
  }

  /** Overlay only the associated image, waiting for matching revisions when both sources share a canvas. */
  #publishPixels(): void {
    if (!this.#alive) return;
    const source = this.#source;
    if (this.binding.loader.kind === 'image') {
      if (source?.kind === 'pixels')
        this.publish({
          kind: 'pixels',
          pixels: source.pixels,
          background: null,
          frame: source.canvas?.document ?? null,
        });
      else if (source) this.publish(source);
      return;
    }
    const image = this.#image;
    if (image?.kind === 'error') {
      this.#edited.invalidate();
      this.publish({ kind: 'black' });
      return;
    }
    if (image?.kind !== 'pixels') {
      this.#edited.invalidate();
      this.publish({ kind: 'loading' });
      return;
    }
    if (source?.kind === 'error') {
      this.#edited.invalidate();
      this.publish(source);
      return;
    }
    if (source?.kind !== 'pixels') {
      this.#edited.invalidate();
      this.publish({ kind: 'loading' });
      return;
    }
    if (
      source.canvas &&
      image.canvas &&
      source.canvas.id === image.canvas.id &&
      (source.canvas.revision !== image.canvas.revision ||
        !sameCanvasSourceDocument(source.canvas.document, image.canvas.document))
    ) {
      this.#edited.invalidate();
      this.publish({ kind: 'loading' });
      return;
    }
    const settings = this.binding.state.edits ?? DEFAULT_MASK_EDITS;
    if (this.association?.editable() && hasMaskEdits(settings)) {
      this.#edited.paint(
        source.pixels,
        image.pixels,
        settings,
        this.mode(),
        this.maximumSize,
        image.canvas?.document ?? null,
      );
      return;
    }
    this.#edited.invalidate();
    this.publish({
      kind: 'pixels',
      pixels: source.pixels,
      background: image.pixels,
      frame: image.canvas?.document ?? null,
    });
  }

  /** Preserve full-resolution coverage for pixel-radius edits while ordinary previews remain display sized. */
  #captureSize(): number {
    return this.binding.loader.kind === 'mask' &&
      this.association?.editable() &&
      hasMaskEdits(this.binding.state.edits ?? DEFAULT_MASK_EDITS)
      ? 0
      : this.maximumSize;
  }
}
