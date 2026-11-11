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
/** Prepare one decoded image/mask pair with the same effective coverage used by live previews. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { MaskEditSettings } from '../domain/MaskEditSettings.js';
import { cropInputRaster, type EditedMaskInput } from '../domain/MaskInputEditing.js';
import type { MaskEditingPort } from './MaskEditingPort.js';
import { InputRasterTransformCache } from './InputRasterTransformCache.js';
import { readMaskEditSettings } from '../domain/MaskEditSettings.js';

/** Carry the coordinated source image and its immutable return geometry together. */
export interface PreparedMaskPair {
  readonly image: CanvasPixelRaster;
  readonly mask: EditedMaskInput;
}

/** Keep image and mask geometry coordinated without involving a particular canvas adapter. */
export class MaskInputPreparation {
  readonly #cache = new InputRasterTransformCache<PreparedMaskPair>();
  /** Share the processing owner already used by mounted mask previews. */
  constructor(private readonly editing: MaskEditingPort) {}

  /** Freeze coverage before cropping the associated image in the same source frame. */
  async prepare(
    image: CanvasPixelRaster,
    mask: CanvasPixelRaster,
    settings: MaskEditSettings,
  ): Promise<PreparedMaskPair> {
    if (
      image.format !== 'rgba8' ||
      mask.format !== 'coverage8' ||
      image.width !== mask.width ||
      image.height !== mask.height
    )
      throw new Error('The image and mask need one common input frame for editing.');
    const accepted = readMaskEditSettings(settings);
    return this.#cache.read(
      [image, mask],
      JSON.stringify(accepted),
      async () => {
        const edited = await this.editing.prepare(mask, accepted, 'submission');
        return Object.freeze({ image: cropInputRaster(image, edited.bounds), mask: edited });
      },
      (value) => (value.image.data.length + value.mask.mask.data.length) * 2,
    );
  }
}
