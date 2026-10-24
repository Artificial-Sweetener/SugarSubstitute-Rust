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
/** Materialize validated portable PNG payloads in plugin-owned temporary storage. */
import { storage } from 'uxp';
/** Describe trusted file entries returned by UXP's plugin-owned temporary storage. */
interface TemporaryImageFile {
  write(data: ArrayBuffer, options: { format: symbol }): Promise<void>;
  delete(): Promise<void>;
}
/** Supply the documented storage factory absent from the pinned namespace declarations. */
interface TemporaryImageStorage {
  localFileSystem: {
    getTemporaryFolder(): Promise<{ createFile(name: string): Promise<TemporaryImageFile> }>;
    createSessionToken(file: TemporaryImageFile): string;
  };
  formats: { binary: symbol };
}
// Native proof verifies this factory; the pinned UXP namespace omits localFileSystem and File.delete.
const temporaryStorage = storage as unknown as TemporaryImageStorage;
import type { CanvasEncodedImage } from '../../../contracts/CanvasProtocol.generated.js';
import { decodeCanvasImage } from '../../../shared/CanvasImageEncoding.js';
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type { PreparedCanvasImage } from '../application/InsertCanvasImage.js';
import { canvasPngDimensions } from '../../../shared/CanvasImagePolicy.js';
/** Own temporary file identity without granting native adapters network or arbitrary filesystem access. */
export class PhotoshopImageAssets {
  readonly #files = new Map<string, TemporaryImageFile>();
  /** Grant Photoshop access to exactly one plugin-owned image without exposing filesystem paths. */
  token(key: string): string {
    const file = this.#files.get(key);
    if (!file) throw new Error('The prepared canvas image is unavailable.');
    return temporaryStorage.localFileSystem.createSessionToken(file);
  }
  /** Decode and admit the portable payload before creating temporary native material. */
  async prepare(
    image: CanvasEncodedImage,
    cancellation: CanvasCancellationSource,
  ): Promise<PreparedCanvasImage> {
    if (cancellation.cancelled()) throw new Error('Canvas image preparation cancelled.');
    const bytes = decodeCanvasImage(image);
    const dimensions = canvasPngDimensions(bytes);
    const key = crypto.randomUUID();
    const folder = await temporaryStorage.localFileSystem.getTemporaryFolder();
    const file = await folder.createFile(`sugarcubes-${key}.png`);
    try {
      await file.write(new Uint8Array(bytes).buffer, { format: temporaryStorage.formats.binary });
    } catch (error: unknown) {
      try {
        await file.delete();
      } catch (cleanup: unknown) {
        throw new AggregateError([error, cleanup], 'Canvas image preparation and cleanup failed.');
      }
      throw error;
    }
    this.#files.set(key, file);
    return {
      key,
      ...dimensions,
      dispose: async () => {
        this.#files.delete(key);
        await file.delete();
      },
    };
  }
}
