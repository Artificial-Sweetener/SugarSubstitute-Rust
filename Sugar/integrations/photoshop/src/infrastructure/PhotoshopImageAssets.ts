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
/** Prepare native sRGB pixel memory outside document mutation and dispose it after delivery. */
import { imaging } from 'photoshop';
import type { CanvasEncodedImage } from '../../../contracts/CanvasProtocol.generated.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type { PreparedCanvasImage } from '../application/InsertCanvasImage.js';
import { decodeCanvasImage } from '../../../shared/CanvasImageEncoding.js';
import { photoshopPngPixels } from './PhotoshopPngPixels.js';

type NativeImageData = Awaited<ReturnType<typeof imaging.createImageDataFromBuffer>>;

/** Own bounded pixel buffers without temporary documents or filesystem materialization. */
export class PhotoshopImageAssets {
  readonly #images = new Map<string, NativeImageData>();

  /** Borrow prepared pixels only during their application's insertion lifetime. */
  imageData(key: string): NativeImageData {
    const image = this.#images.get(key);
    if (!image) throw new Error('The prepared canvas pixels are unavailable.');
    return image;
  }

  /** Validate compressed and decoded geometry before acquiring native pixel memory. */
  async prepare(
    image: CanvasEncodedImage,
    cancellation: CanvasCancellationSource,
  ): Promise<PreparedCanvasImage> {
    if (cancellation.cancelled()) throw new Error('Canvas image preparation cancelled.');
    const { width, height, pixels } = photoshopPngPixels(decodeCanvasImage(image));
    const data = await imaging.createImageDataFromBuffer(pixels, {
      width,
      height,
      components: 4,
      chunky: true,
      colorSpace: 'RGB',
      colorProfile: 'sRGB IEC61966-2.1',
    });
    const key = crypto.randomUUID();
    this.#images.set(key, data);
    return {
      key,
      width,
      height,
      dispose: async () => {
        this.#images.delete(key);
        await data.dispose();
      },
    };
  }
}
