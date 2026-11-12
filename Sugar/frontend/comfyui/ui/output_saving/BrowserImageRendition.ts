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
/** Encode a bounded retained original through the browser's existing image codec. */
import type { ImageDimensions } from '../execution/domain/WorkflowOutput.js';
import { outputImageTransferDimensions } from './OutputImageDimensions.js';
import { MAX_CANVAS_IMAGE_BYTES } from '../../../../integrations/shared/CanvasImagePolicy.js';

/** Keep a browser codec behind an explicit adapter boundary for retained image copy actions. */
export interface OutputImageRendition {
  encode(
    blob: Blob,
    mime: 'image/png' | 'image/jpeg',
    quality: number,
    dimensions: ImageDimensions,
    signal?: AbortSignal,
  ): Promise<Blob>;
}

/** Retain each bitmap until real conversion completes and release every temporary canvas allocation. */
export class BrowserImageRendition implements OutputImageRendition {
  #active = 0;
  /** Bind the current document without constructing native widgets or storing image pixels. */
  constructor(private readonly document: Document) {}

  /** Encode caller-verified bounded headers; cancellation still observes bitmap/codec terminal return. */
  async encode(
    blob: Blob,
    mime: 'image/png' | 'image/jpeg',
    quality: number,
    dimensions: ImageDimensions,
    signal?: AbortSignal,
  ): Promise<Blob> {
    if (signal?.aborted) throw new DOMException('Cancelled', 'AbortError');
    if (this.#active >= 2) throw new Error('Output image conversion is busy.');
    if (
      !['image/png', 'image/jpeg', 'image/webp'].includes(blob.type) ||
      !blob.size ||
      blob.size > MAX_CANVAS_IMAGE_BYTES ||
      !outputImageTransferDimensions(dimensions) ||
      !Number.isFinite(quality) ||
      quality < 0.01 ||
      quality > 1
    )
      throw new Error('Output image conversion exceeds its bounds.');
    const canvas = this.document.createElement('canvas');
    this.#active++;
    let bitmap: ImageBitmap | undefined;
    try {
      bitmap = await createImageBitmap(blob);
      if (signal?.aborted) throw new DOMException('Cancelled', 'AbortError');
      // Browsers apply JPEG EXIF rotation; an orthogonal transpose preserves the encoded geometry.
      if (
        !(
          (bitmap.width === dimensions.width && bitmap.height === dimensions.height) ||
          (bitmap.width === dimensions.height && bitmap.height === dimensions.width)
        ) ||
        !outputImageTransferDimensions(bitmap)
      )
        throw new Error('Decoded output geometry differs from its confirmed header.');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Output image encoding is unavailable.');
      context.drawImage(bitmap, 0, 0);
      const encoded = await new Promise<Blob>((resolve, reject) =>
        canvas.toBlob(
          (value) => (value ? resolve(value) : reject(new Error('Output image encoding failed.'))),
          mime,
          quality,
        ),
      );
      if (signal?.aborted) throw new DOMException('Cancelled', 'AbortError');
      if (encoded.type !== mime || !encoded.size || encoded.size > MAX_CANVAS_IMAGE_BYTES)
        throw new Error('Encoded output rendition is invalid.');
      return encoded;
    } finally {
      bitmap?.close();
      canvas.width = canvas.height = 0;
      this.#active--;
    }
  }
}
