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
/** Reuse immutable raster work across captures while bounding retained source and result memory. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';

interface Entry<T> {
  readonly sources: readonly CanvasPixelRaster[];
  readonly parameters: string;
  readonly promise: Promise<T>;
  bytes: number;
}

/** Identify transformations by actual pixels and parameters rather than transient capture objects. */
export class InputRasterTransformCache<T> {
  readonly #ready: Entry<T>[] = [];
  readonly #pending = new Set<Entry<T>>();
  #bytes = 0;
  #epoch = 0;

  /** Keep cache limits injectable for deterministic eviction and admission proofs. */
  constructor(
    private readonly budget = MAX_CANVAS_RASTER_BYTES,
    private readonly limit = 16,
  ) {}

  /** Share pending work and reuse completed work only when every input pixel and parameter matches. */
  read(
    sources: readonly CanvasPixelRaster[],
    parameters: string,
    create: () => Promise<T>,
    resultBytes: (value: T) => number,
  ): Promise<T> {
    const matches = (entry: Entry<T>): boolean =>
      entry.parameters === parameters &&
      entry.sources.length === sources.length &&
      entry.sources.every((source, index) => {
        const candidate = sources[index];
        return (
          candidate !== undefined &&
          source.width === candidate.width &&
          source.height === candidate.height &&
          source.format === candidate.format &&
          source.data === candidate.data
        );
      });
    const readyIndex = this.#ready.findIndex(matches);
    if (readyIndex >= 0) {
      const ready = this.#ready.splice(readyIndex, 1)[0];
      if (ready) {
        this.#ready.push(ready);
        return ready.promise;
      }
    }
    for (const pending of this.#pending) if (matches(pending)) return pending.promise;
    if (this.#pending.size >= this.limit)
      return Promise.reject(new Error('Too many pending raster transformations.'));
    const epoch = this.#epoch;
    let work: Promise<T>;
    try {
      work = create();
    } catch (error: unknown) {
      return Promise.reject(
        error instanceof Error
          ? error
          : new Error('Raster transformation failed.', { cause: error }),
      );
    }
    const entry: Entry<T> = {
      sources: sources.map((source) => Object.freeze({ ...source })),
      parameters,
      bytes: sources.reduce((sum, source) => sum + source.data.length * 2, 0),
      promise: work
        .then((value) => {
          entry.bytes += resultBytes(value);
          if (epoch === this.#epoch && entry.bytes <= this.budget) {
            this.#ready.push(entry);
            this.#bytes += entry.bytes;
            while (this.#ready.length > this.limit || this.#bytes > this.budget) {
              const oldest = this.#ready.shift();
              if (oldest) this.#bytes -= oldest.bytes;
            }
          }
          return value;
        })
        .finally(() => this.#pending.delete(entry)),
    };
    this.#pending.add(entry);
    return entry.promise;
  }

  /** Retire retained pixels and fence late completions when their owning session ends. */
  clear(): void {
    this.#epoch++;
    this.#ready.length = 0;
    this.#bytes = 0;
    this.#pending.clear();
  }
}
