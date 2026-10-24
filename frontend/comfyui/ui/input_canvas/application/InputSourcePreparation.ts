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
/** Stage captured sources as immutable loader images using the same representation as consumer previews. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { canvasInputSourceKey } from '../../core/embedded/CanvasRuntime.js';
import type { CanvasSourceCapture, CanvasSourceEntry } from './CanvasSourceCapture.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';

/** Keep file encoding and upload outside application source interpretation. */
export interface InputSourceAssets {
  stage(pixels: CanvasPixelRaster): Promise<string>;
}

/** Adapt source capture to a file-loading field without changing source channels or the loader's settings. */
export class InputSourcePreparation {
  /** Reuse the generic capture owner also used by committed preview updates. */
  constructor(
    private readonly capture: CanvasSourceCapture,
    private readonly assets: InputSourceAssets,
  ) {}
  /** Capture all targets before upload and preserve each caller's exact entry identity. */
  async prepare(entries: readonly CanvasSourceEntry[]): Promise<ReadonlyMap<string, string>> {
    const snapshots = await this.capture.capture(entries, 0),
      staged = new Map<string, Promise<string>>(),
      files = new Map<string, string>();
    for (const entry of entries) {
      const snapshot = snapshots.get(entry.key);
      if (!snapshot) throw new Error('Missing captured input source.');
      const key = JSON.stringify([canvasInputSourceKey(entry.source), snapshot.pixels.format]);
      let file = staged.get(key);
      if (!file) {
        file = this.assets.stage(maskSourceImage(snapshot.pixels));
        staged.set(key, file);
      }
      files.set(entry.key, await file);
    }
    return files;
  }
}
