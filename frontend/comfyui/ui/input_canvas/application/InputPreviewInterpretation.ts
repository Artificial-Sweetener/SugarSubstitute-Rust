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
/** Interpret consumer previews separately from source binding state and native capture policy. */
import type { InputLoader, InputPreviewSettings } from '../domain/InputLoader.js';
import { usesNativeMaskChannels } from '../domain/InputLoader.js';
import type { InputPreviewPort } from './InputEditorPorts.js';
import type { CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { interpretMaskSource } from '../domain/MaskSourcePixels.js';
/** Apply mask semantics while image consumers retain actual source content and transparency. */
export class InputPreviewInterpretation {
  readonly #interpreted = new WeakMap<
    CanvasPixelRaster,
    { key: string; pixels: Promise<CanvasPixelRaster> }
  >();
  /** Read current authored settings from their owning node only when interpretation is requested. */
  constructor(
    private readonly loader: InputLoader,
    private readonly files: InputPreviewPort,
    private readonly readSettings: () => InputPreviewSettings,
  ) {}
  /** Expose a current immutable settings snapshot for view invalidation. */
  get settings(): InputPreviewSettings {
    return this.readSettings();
  }
  /** Preserve custom-loader interpretation at native resolution while bounding ordinary display capture. */
  captureSize(maximumSize: number): number {
    return this.loader.kind === 'image' || usesNativeMaskChannels(this.loader) ? maximumSize : 0;
  }
  /** Decode an actual file without competing with its loader's file codec. */
  file(file: string, maximumSize: number): Promise<CanvasPixelRaster> {
    return this.files.load(file, this.settings, maximumSize);
  }
  /** Reinterpret raw mask channels locally while image capture already supplies selected image pixels. */
  canvas(source: CanvasPixelRaster, maximumSize: number): Promise<CanvasPixelRaster> {
    const settings = this.settings,
      key = JSON.stringify([settings, maximumSize]);
    const cached = this.#interpreted.get(source);
    if (cached?.key === key) return cached.pixels;
    const pixels = this.#interpret(source, settings, maximumSize);
    this.#interpreted.set(source, { key, pixels });
    void pixels.catch(() => {
      if (this.#interpreted.get(source)?.pixels === pixels) this.#interpreted.delete(source);
    });
    return pixels;
  }
  /** Apply the loader's actual settings after sharing identical in-flight interpretation. */
  async #interpret(
    source: CanvasPixelRaster,
    settings: InputPreviewSettings,
    maximumSize: number,
  ): Promise<CanvasPixelRaster> {
    if (this.loader.kind === 'image') {
      if (source.format !== 'rgba8')
        throw new Error('The image source did not contain image pixels.');
      return source;
    }
    if (usesNativeMaskChannels(this.loader)) {
      const channel = settings.inputs.channel;
      if (typeof channel !== 'string')
        throw new Error('The mask channel must be an authored setting to show a live preview.');
      return interpretMaskSource(source, channel);
    }
    return await this.files.interpret(source, settings, maximumSize);
  }
}
