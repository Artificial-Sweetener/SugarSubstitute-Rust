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
/** Load exact Substitute artwork and cache complete accent-tinted orb interaction variants. */
import { orbStateAccent, tintOrbPixel, type OrbRgb } from './OrbTint.js';

/** Compose glass layers around the original witch icon outside pointer-event hot paths. */
export class AppOrbRenderer {
  readonly canvas: HTMLCanvasElement;
  readonly #document: Document;
  #layers: readonly HTMLImageElement[] | null = null;
  #variants: ReadonlyMap<string, HTMLCanvasElement> = new Map();
  #signature = '';
  #revision = 0;
  #state: 'idle' | 'hover' | 'pressed' = 'idle';
  #disposed = false;

  /** Create a high-quality visual surface; the enclosing Fluent button owns interaction semantics. */
  constructor(documentRef: Document) {
    this.#document = documentRef;
    this.canvas = documentRef.createElement('canvas');
    this.canvas.width = this.canvas.height = 256;
    this.canvas.setAttribute('aria-hidden', 'true');
  }

  /** Prepare all pointer-state variants only when the accepted palette or DPI changes. */
  async update(color: string, dark: boolean): Promise<void> {
    const signature = `${color}:${dark}:${this.#document.defaultView?.devicePixelRatio ?? 1}`;
    if (signature === this.#signature || this.#disposed) return;
    this.#signature = signature;
    const revision = ++this.#revision;
    const layers =
      this.#layers ??
      (await Promise.all(
        ['orb_base.png', 'app_icon_256.png', 'orb_lower_overlay.png', 'orb_upper_overlay.png'].map(
          (filename) => this.#load(filename),
        ),
      ));
    if (this.#disposed || revision !== this.#revision) return;
    this.#layers = layers;
    const accent: OrbRgb = [
      Number.parseInt(color.slice(1, 3), 16),
      Number.parseInt(color.slice(3, 5), 16),
      Number.parseInt(color.slice(5, 7), 16),
    ];
    const variants = new Map<string, HTMLCanvasElement>();
    for (const state of ['idle', 'hover', 'pressed'] as const)
      variants.set(state, this.#compose(layers, orbStateAccent(accent, state)));
    this.#variants = variants;
    this.setState(this.#state);
  }

  /** Blit a prepared interaction state without decoding assets or retinting pixels. */
  setState(state: 'idle' | 'hover' | 'pressed'): void {
    this.#state = state;
    const image = this.#variants.get(state);
    if (!image || this.#disposed) return;
    const context = this.#context(this.canvas);
    context.clearRect(0, 0, 256, 256);
    context.drawImage(image, 0, 0);
  }

  /** Invalidate pending asset work and release cached raster variants. */
  dispose(): void {
    this.#disposed = true;
    this.#revision++;
    this.#layers = null;
    this.#variants = new Map();
  }

  /** Load a pinned packaged asset and report decode failures to the surface owner. */
  async #load(filename: string): Promise<HTMLImageElement> {
    const image = this.#document.createElement('img');
    image.src = new URL(`../../../../orb/${filename}`, import.meta.url).href;
    await image.decode();
    return image;
  }

  /** Preserve the original PSD-relative icon placement between base and overlay layers. */
  #compose(layers: readonly HTMLImageElement[], accent: OrbRgb): HTMLCanvasElement {
    const output = this.#document.createElement('canvas');
    output.width = output.height = 256;
    const target = this.#context(output);
    target.imageSmoothingEnabled = true;
    target.imageSmoothingQuality = 'high';
    for (const [index, image] of layers.entries()) {
      if (index === 1) {
        target.drawImage(
          image,
          (256 * 109) / 1146,
          (256 * 98) / 1160,
          (256 * 928) / 1146,
          (256 * 928) / 1160,
        );
        continue;
      }
      const layer = this.#document.createElement('canvas');
      layer.width = layer.height = 256;
      const context = this.#context(layer);
      context.drawImage(image, 0, 0, 256, 256);
      const data = context.getImageData(0, 0, 256, 256);
      for (let offset = 0; offset < data.data.length; offset += 4) {
        const tinted = tintOrbPixel(
          [
            data.data[offset] ?? 0,
            data.data[offset + 1] ?? 0,
            data.data[offset + 2] ?? 0,
            data.data[offset + 3] ?? 0,
          ],
          accent,
        );
        data.data.set(tinted, offset);
      }
      context.putImageData(data, 0, 0);
      target.drawImage(layer, 0, 0);
    }
    return output;
  }

  /** Require browser raster support rather than silently substituting different artwork. */
  #context(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Substitute orb rendering is unavailable.');
    return context;
  }
}
