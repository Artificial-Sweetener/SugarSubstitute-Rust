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
import type { LoraMediaDecoder, LoraMediaReference, LoraRaster } from './lora-media-contracts.js';

/** Bound retained rasters to 65,536 pixels and 512 pixels on either edge, preserving their source aspect ratio. */
function rasterSize(width: number, height: number): { width: number; height: number } {
  const scale = Math.min(1, 512 / Math.max(width, height), Math.sqrt(65_536 / (width * height)));
  return {
    width: Math.max(1, Math.floor(width * scale)),
    height: Math.max(1, Math.floor(height * scale)),
  };
}

/** How many decoded images are kept after the last chip showing them has gone. */
export const IDLE_LORA_RASTERS = 256;

/** One decoded image, shared by every chip in every prompt that shows its address. */
interface SharedBitmap {
  readonly key: string;
  readonly bitmap: ImageBitmap;
  users: number;
}

/**
 * Decoded images by address and size. Addresses are content hashes served as immutable, so one
 * decode serves every prompt, and a chip that returns to the screen is painted from the kept
 * pixels. An image nobody shows stays kept, oldest released first out once more than
 * {@link IDLE_LORA_RASTERS} are idle.
 */
const shared = new Map<string, SharedBitmap>();
const idle = new Map<string, SharedBitmap>();
const decoding = new Map<string, Promise<SharedBitmap | null>>();

/** Close and forget every kept image; for tests and for a page that is being torn down. */
export function forgetLoraRasters(): void {
  for (const entry of shared.values()) entry.bitmap.close();
  shared.clear();
  idle.clear();
  decoding.clear();
}

function retain(entry: SharedBitmap): BrowserLoraRaster {
  entry.users++;
  idle.delete(entry.key);
  return new BrowserLoraRaster(entry);
}

function release(entry: SharedBitmap): void {
  if (--entry.users > 0) return;
  idle.set(entry.key, entry);
  for (const [key, oldest] of idle) {
    if (idle.size <= IDLE_LORA_RASTERS) break;
    idle.delete(key);
    shared.delete(key);
    oldest.bitmap.close();
  }
}

/** One chip's hold on a shared decoded image; the pixels close when the last hold and the cache let go. */
class BrowserLoraRaster implements LoraRaster {
  readonly width: number;
  readonly height: number;
  private entry: SharedBitmap | null;
  /** Hold decoded pixels; the temporary image and its URL reference have already been released. */
  constructor(entry: SharedBitmap) {
    this.entry = entry;
    this.width = entry.bitmap.width;
    this.height = entry.bitmap.height;
  }
  /** Cover a stable banner with centered source pixels; DOM construction starts no image request or decode. */
  paint(canvas: HTMLCanvasElement): void {
    if (!this.entry) throw new Error('LoRA raster already disposed');
    const context = canvas.getContext('2d');
    if (!context) throw new Error('LoRA canvas unavailable');
    const scale = Math.max(canvas.width / this.width, canvas.height / this.height);
    const width = canvas.width / scale,
      height = canvas.height / scale;
    context.drawImage(
      this.entry.bitmap,
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
  /** Idempotent release gives up this hold and forbids further painting. */
  dispose(): void {
    if (this.entry) release(this.entry);
    this.entry = null;
  }
}

/** Decode one address once for everyone waiting on it; no single waiter can cancel the others. */
function decodeShared(key: string, reference: LoraMediaReference): Promise<SharedBitmap | null> {
  const running = decoding.get(key);
  if (running) return running;
  const work = (async (): Promise<SharedBitmap | null> => {
    const image = new Image();
    image.referrerPolicy = 'no-referrer';
    try {
      image.src = reference.url;
      await image.decode();
      if (image.naturalWidth !== reference.width || image.naturalHeight !== reference.height)
        return null;
      const size = rasterSize(image.naturalWidth, image.naturalHeight);
      const bitmap = await createImageBitmap(image, {
        resizeWidth: size.width,
        resizeHeight: size.height,
        resizeQuality: 'high',
      });
      const entry: SharedBitmap = { key, bitmap, users: 0 };
      shared.set(key, entry);
      idle.set(key, entry);
      return entry;
    } catch (cause) {
      if (cause instanceof DOMException) return null;
      throw cause;
    } finally {
      // Removing the URL releases the temporary image; the host alone owns URL revocation.
      image.removeAttribute('src');
    }
  })();
  decoding.set(key, work);
  void work.then(
    () => decoding.delete(key),
    () => decoding.delete(key),
  );
  return work;
}

/** Expected browser decode/bitmap failures become explicit unavailable media; other failures remain observable. */
export const decodeLoraMedia: LoraMediaDecoder = async (reference, signal) => {
  if (signal.aborted) return null;
  const key = `${reference.url}|${String(reference.width)}x${String(reference.height)}`;
  const kept = shared.get(key);
  if (kept) return retain(kept);
  const entry = await decodeShared(key, reference);
  // A waiter that was cancelled takes no hold; the decoded image stays kept for the next one.
  return entry && !signal.aborted ? retain(entry) : null;
};
