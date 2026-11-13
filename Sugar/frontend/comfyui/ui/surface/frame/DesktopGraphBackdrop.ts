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
/** Let the desktop body wash show through Comfy's graph while keeping Comfy's own grid lines. */
import { isRecord } from '../../types/common.js';

/** LiteGraph fields Comfy's palette loader assigns on every palette or background change. */
const CLEAR_COLOR = 'clear_background_color';
const GRID_IMAGE = 'background_image';
const GRID_PATTERN = '_pattern';

/** Turn one palette grid tile into a translucent tile for the given theme; rejects undecodable images. */
export type GridTileDeriver = (source: string, darkTheme: boolean) => Promise<string>;

export interface DesktopGraphBackdropOptions {
  readonly deriveGrid: GridTileDeriver;
  report(error: unknown): void;
}

interface ComfyBackground {
  clear: unknown;
  image: unknown;
}

/**
 * Opacity of the strongest grid line before LiteGraph fades the whole grid with zoom (it draws the
 * tile at half this strength at the default zoom). At that zoom a major line then differs from the
 * wash by roughly 40 of 255 levels on either theme's body wash and a minor line by two thirds of
 * that: clearly visible without competing with nodes.
 */
export const GRID_LINE_PEAK_ALPHA = 0.36;

/**
 * Re-express an opaque grid tile as lines alone: the tile's dominant field color becomes fully
 * transparent, so the wash painted beneath the canvas carries Mica, and every other pixel becomes
 * ink in proportion to how far it departs from the field. The ink is white on the dark wash and
 * black on the light wash, whichever way the palette's own lines depart from its field: the dark
 * palette draws lines darker than its field, the light palette lighter, and neither stays visible
 * on a translucent wash. The strongest line pixel receives {@link GRID_LINE_PEAK_ALPHA}.
 */
export function translucentGridPixels(
  pixels: Uint8ClampedArray,
  darkTheme: boolean,
): Uint8ClampedArray {
  const at = (index: number): number => pixels[index] ?? 0;
  const counts = new Map<number, number>();
  for (let index = 0; index + 3 < pixels.length; index += 4) {
    const key = (at(index) << 16) | (at(index + 1) << 8) | at(index + 2);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  let field = 0;
  let fieldCount = -1;
  for (const [key, count] of counts)
    if (count > fieldCount) {
      field = key;
      fieldCount = count;
    }
  const base = [(field >> 16) & 255, (field >> 8) & 255, field & 255] as const;
  const departures = new Float32Array(pixels.length / 4);
  let strongest = 0;
  for (let index = 0; index + 3 < pixels.length; index += 4) {
    let departure = 0;
    base.forEach((fieldChannel, channel) => {
      departure = Math.max(departure, Math.abs(at(index + channel) - fieldChannel));
    });
    departures[index / 4] = departure;
    strongest = Math.max(strongest, departure);
  }
  const output = new Uint8ClampedArray(pixels.length);
  const ink = darkTheme ? 255 : 0;
  for (let index = 0; index + 3 < pixels.length; index += 4) {
    const strength = strongest === 0 ? 0 : (departures[index / 4] ?? 0) / strongest;
    output[index] = output[index + 1] = output[index + 2] = ink;
    output[index + 3] = Math.round(strength * GRID_LINE_PEAK_ALPHA * (at(index + 3) / 255) * 255);
  }
  return output;
}

/** Decode a grid tile in the page and encode its translucent replacement as a PNG data URL. */
export function browserGridTileDeriver(document: Document): GridTileDeriver {
  return (source, darkTheme) =>
    new Promise<string>((resolve, reject) => {
      const image = new Image();
      image.addEventListener('error', () => reject(new Error('Comfy grid tile did not decode.')));
      image.addEventListener('load', () => {
        const canvas = document.createElement('canvas');
        canvas.width = image.naturalWidth;
        canvas.height = image.naturalHeight;
        const context = canvas.getContext('2d');
        if (!context || canvas.width === 0 || canvas.height === 0) {
          reject(new Error('Comfy grid tile has no drawable raster.'));
          return;
        }
        context.drawImage(image, 0, 0);
        const tile = context.getImageData(0, 0, canvas.width, canvas.height);
        tile.data.set(translucentGridPixels(tile.data, darkTheme));
        context.putImageData(tile, 0, 0);
        resolve(canvas.toDataURL('image/png'));
      });
      image.src = source;
    });
}

/**
 * Own LiteGraph's background projection for one desktop frame lifetime. Comfy keeps assigning its
 * palette values; this owner records them and presents a transparent clear color plus a
 * translucent copy of Comfy's grid, so the CSS body wash beneath the graph canvas carries Mica.
 * Detaching restores Comfy's latest assignments as plain data properties.
 */
export class DesktopGraphBackdrop {
  #canvas: Record<string, unknown> | null = null;
  #comfy: ComfyBackground = { clear: undefined, image: undefined };
  #grids = new Map<string, string>();
  #pending = new Set<string>();
  #dark = true;

  constructor(private readonly options: DesktopGraphBackdropOptions) {}

  /**
   * Bind the current Comfy canvas once; repeated calls with the same canvas are idempotent. The
   * theme chooses the grid ink and may change between calls, which rebuilds the presented grid.
   */
  attach(app: unknown, darkTheme: boolean): void {
    const canvas = isRecord(app) && isRecord(app.canvas) ? app.canvas : null;
    if (canvas === this.#canvas) {
      if (canvas) this.#follow(darkTheme);
      return;
    }
    this.detach();
    if (!canvas) return;
    this.#dark = darkTheme;
    this.#canvas = canvas;
    this.#comfy = { clear: canvas[CLEAR_COLOR], image: canvas[GRID_IMAGE] };
    Object.defineProperty(canvas, CLEAR_COLOR, {
      configurable: true,
      enumerable: true,
      get: () => 'transparent',
      set: (value: unknown) => {
        this.#comfy.clear = value;
      },
    });
    Object.defineProperty(canvas, GRID_IMAGE, {
      configurable: true,
      enumerable: true,
      get: () => this.#presentedGrid(),
      set: (value: unknown) => {
        this.#comfy.image = value;
        this.#prepare(value);
      },
    });
    this.#prepare(this.#comfy.image);
    this.#redraw();
  }

  /** Return Comfy's own background values to its canvas without discarding later assignments. */
  detach(): void {
    const canvas = this.#canvas;
    if (!canvas) return;
    this.#canvas = null;
    for (const [name, value] of [
      [CLEAR_COLOR, this.#comfy.clear],
      [GRID_IMAGE, this.#comfy.image],
    ] as const)
      Object.defineProperty(canvas, name, {
        configurable: true,
        enumerable: true,
        writable: true,
        value,
      });
    this.#redrawCanvas(canvas);
  }

  /** Adopt a changed theme and redraw once the grid inked for it is ready. */
  #follow(darkTheme: boolean): void {
    if (darkTheme === this.#dark) return;
    this.#dark = darkTheme;
    this.#prepare(this.#comfy.image);
    this.#redraw();
  }

  /** Cache derived tiles per theme, since the same palette tile is inked differently by each. */
  #key(source: string, darkTheme: boolean): string {
    return `${darkTheme ? 'dark' : 'light'}:${source}`;
  }

  /** Show only a derived translucent tile; Comfy's opaque tile would hide the wash. */
  #presentedGrid(): string | null {
    const source = this.#comfy.image;
    return typeof source === 'string'
      ? (this.#grids.get(this.#key(source, this.#dark)) ?? null)
      : null;
  }

  #prepare(source: unknown): void {
    if (typeof source !== 'string' || !source) return;
    const dark = this.#dark;
    const key = this.#key(source, dark);
    if (this.#grids.has(key) || this.#pending.has(key)) return;
    this.#pending.add(key);
    this.options.deriveGrid(source, dark).then(
      (grid) => {
        this.#pending.delete(key);
        this.#grids.set(key, grid);
        if (this.#comfy.image === source && this.#dark === dark) this.#redraw();
      },
      (error: unknown) => {
        this.#pending.delete(key);
        this.options.report(error);
      },
    );
  }

  #redraw(): void {
    if (this.#canvas) this.#redrawCanvas(this.#canvas);
  }

  /** Drop LiteGraph's cached pattern so the next background pass rebuilds it from the tile. */
  #redrawCanvas(canvas: Record<string, unknown>): void {
    canvas[GRID_PATTERN] = undefined;
    const setDirty = canvas.setDirty;
    if (typeof setDirty === 'function') setDirty.call(canvas, true, true);
  }
}
