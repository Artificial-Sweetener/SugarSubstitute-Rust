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
/** Decode a host-acquired browser image outside CodeMirror input, measurement and paint. */
import type { LoraMediaDecoder, LoraRaster } from './lora-media-contracts.js';

/** Bound retained rasters to 65,536 pixels and 512 pixels on either edge, preserving their source aspect ratio. */
function rasterSize(width: number, height: number): { width: number; height: number } {
  const scale = Math.min(1, 512 / Math.max(width, height), Math.sqrt(65_536 / (width * height)));
  return {
    width: Math.max(1, Math.floor(width * scale)),
    height: Math.max(1, Math.floor(height * scale)),
  };
}

/** Browser-owned bitmap lifetime is separate from the host-owned URL lifetime. */
class BrowserLoraRaster implements LoraRaster {
  readonly width: number;
  readonly height: number;
  private bitmap: ImageBitmap | null;
  /** Retain only decoded pixels; the temporary image and its URL reference have already been released. */
  constructor(bitmap: ImageBitmap) {
    this.bitmap = bitmap;
    this.width = bitmap.width;
    this.height = bitmap.height;
  }
  /** Cover a stable banner with centered source pixels; DOM construction starts no image request or decode. */
  paint(canvas: HTMLCanvasElement): void {
    if (!this.bitmap) throw new Error('LoRA raster already disposed');
    const context = canvas.getContext('2d');
    if (!context) throw new Error('LoRA canvas unavailable');
    const scale = Math.max(canvas.width / this.width, canvas.height / this.height);
    const width = canvas.width / scale,
      height = canvas.height / scale;
    context.drawImage(
      this.bitmap,
      (this.width - width) / 2,
      (this.height - height) / 2,
      width,
      height,
      0,
      0,
      canvas.width,
      canvas.height,
    );
  }
  /** Idempotent release closes the native bitmap and forbids further painting. */
  dispose(): void {
    this.bitmap?.close();
    this.bitmap = null;
  }
}

/** Expected browser decode/bitmap failures become explicit unavailable media; other failures remain observable. */
export const decodeLoraMedia: LoraMediaDecoder = async (reference, signal) => {
  if (signal.aborted) return null;
  const image = new Image();
  image.referrerPolicy = 'no-referrer';
  /** Removing the URL stops the temporary image request; the host alone owns URL revocation. */
  const cancel = (): void => image.removeAttribute('src');
  signal.addEventListener('abort', cancel, { once: true });
  try {
    image.src = reference.url;
    await image.decode();
    if (signal.aborted) return null;
    if (image.naturalWidth !== reference.width || image.naturalHeight !== reference.height)
      return null;
    const size = rasterSize(image.naturalWidth, image.naturalHeight);
    const bitmap = await createImageBitmap(image, {
      resizeWidth: size.width,
      resizeHeight: size.height,
      resizeQuality: 'high',
    });
    if (signal.aborted) {
      bitmap.close();
      return null;
    }
    return new BrowserLoraRaster(bitmap);
  } catch (cause) {
    if (signal.aborted || cause instanceof DOMException) return null;
    throw cause;
  } finally {
    signal.removeEventListener('abort', cancel);
    cancel();
  }
};
