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
/** Coalesce one mounted mask preview's editing work without replacing its source or geometry owners. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { InputPreviewContentValue } from './InputPreviewContent.js';
import type { MaskEditingPort } from './MaskEditingPort.js';
import type { MaskEditSettings } from '../domain/MaskEditSettings.js';
import type { MaskPreviewMode, MaskPreviewPlacement } from '../domain/MaskPreviewGeometry.js';
import type { EditedMaskInput } from '../domain/MaskInputEditing.js';

interface Intent {
  readonly mask: CanvasPixelRaster;
  readonly image: CanvasPixelRaster;
  readonly settings: MaskEditSettings;
  readonly mode: MaskPreviewMode;
  readonly maximumSize: number;
  readonly frame: { readonly width: number; readonly height: number } | null;
  readonly serial: number;
}

interface Completed {
  readonly mask: CanvasPixelRaster;
  readonly key: string;
  readonly edited: EditedMaskInput;
  readonly placement: MaskPreviewPlacement;
}

/** Retain one active request and the latest lightweight intent rather than processing every slider event. */
export class InputMaskPreview {
  #serial = 0;
  #active = false;
  #latest: Intent | null = null;
  #requested: Intent | null = null;
  #alive = true;
  #completed: Completed | null = null;
  /** Share the same processing port as immutable generation preparation. */
  constructor(
    private readonly editing: MaskEditingPort,
    private readonly publish: (value: InputPreviewContentValue) => void,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Preserve the mounted preview while coalescing edits from unchanged captured source pixels. */
  paint(
    mask: CanvasPixelRaster,
    image: CanvasPixelRaster,
    settings: MaskEditSettings,
    mode: MaskPreviewMode,
    maximumSize: number,
    frame: { readonly width: number; readonly height: number } | null,
  ): void {
    const previous = this.#requested;
    if (
      previous?.mask === mask &&
      previous.image === image &&
      previous.mode === mode &&
      previous.maximumSize === maximumSize &&
      previous.frame?.width === frame?.width &&
      previous.frame?.height === frame?.height &&
      JSON.stringify(previous.settings) === JSON.stringify(settings)
    )
      return;
    const intent = { mask, image, settings, mode, maximumSize, frame, serial: ++this.#serial };
    this.#latest = this.#requested = intent;
    if (this.#completed?.mask === mask && this.#completed.key === JSON.stringify(settings)) {
      this.#latest = null;
      this.#present(intent, this.#completed);
    } else {
      this.publish({ kind: 'loading' });
      this.#drain();
    }
  }

  /** Fence pending results when an unedited or unavailable source takes over the same surface. */
  invalidate(): void {
    this.#serial++;
    this.#latest = this.#requested = null;
  }

  /** Retire presentation before accepting any late processing response. */
  dispose(): void {
    this.#alive = false;
    this.invalidate();
  }

  /** Process the newest admitted intent after each response, without timer-driven retries. */
  #drain(): void {
    if (this.#active || !this.#alive || !this.#latest) return;
    const intent = this.#latest;
    this.#latest = null;
    this.#active = true;
    void this.editing
      .prepare(intent.mask, intent.settings, 'preview')
      .then((edited) => {
        if (!this.#alive) return;
        const completed = {
          mask: intent.mask,
          key: JSON.stringify(intent.settings),
          edited,
          placement: {
            bounds: edited.bounds,
            frame: { width: intent.mask.width, height: intent.mask.height },
          },
        };
        this.#completed = completed;
        const current = this.#requested;
        if (current?.mask === intent.mask && JSON.stringify(current.settings) === completed.key) {
          this.#latest = null;
          this.#present(current, completed);
        }
      })
      .catch((error: unknown) => {
        if (!this.#alive || intent.serial !== this.#serial) return;
        this.publish({
          kind: 'error',
          message: error instanceof Error ? error.message : 'Mask preview unavailable',
        });
        this.report(error);
      })
      .finally(() => {
        this.#active = false;
        this.#drain();
      });
  }
  /** Frame cached display pixels in a viewport instead of making a cropped image for presentation. */
  #present(intent: Intent, completed: Completed): void {
    if (intent.image.width > intent.maximumSize || intent.image.height > intent.maximumSize)
      throw new Error('The image preview exceeds its display limit.');
    const { edited, placement } = completed;
    const input = intent.mode === 'input' && edited.cropped;
    this.publish({
      kind: 'pixels',
      pixels: edited.mask,
      background: intent.image,
      frame: input ? { width: edited.bounds.width, height: edited.bounds.height } : intent.frame,
      placement,
      outline: edited.cropped && !input,
      ...(input ? { viewport: placement } : {}),
    });
  }
}
