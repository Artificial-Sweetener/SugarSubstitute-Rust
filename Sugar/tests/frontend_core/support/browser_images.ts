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
/**
 * jsdom loads and decodes no images, so `HTMLImageElement.decode()` is missing. Tests get one they
 * control: an image decodes at once unless its address was named broken, and decoding can be held
 * so a test sees the moment before an image is ready.
 */
export interface ImageDecoding {
  /** Addresses whose decode fails. */
  readonly broken: Set<string>;
  /** While true, decodes wait for `release`. */
  held: boolean;
  /** Addresses whose decode is waiting. */
  waiting(): string[];
  /** Settle waiting decodes, optionally only those whose address contains `part`. */
  release(part?: string): Promise<void>;
  /** Forget broken addresses and waiting decodes, and stop holding. */
  reset(): void;
}

declare global {
  // A global is declared with var; let and const do not reach globalThis.
  // eslint-disable-next-line no-var
  var imageDecoding: ImageDecoding;
}

interface Waiting {
  readonly url: string;
  settle(): void;
}

const waiting: Waiting[] = [];
const broken = new Set<string>();

globalThis.imageDecoding = {
  broken,
  held: false,
  waiting: () => waiting.map((entry) => entry.url),
  async release(part) {
    for (const entry of waiting.splice(0))
      if (part === undefined || entry.url.includes(part)) entry.settle();
      else waiting.push(entry);
    // Let the settled decodes' continuations and any render they cause run.
    for (let turn = 0; turn < 4; turn++) await Promise.resolve();
  },
  reset() {
    broken.clear();
    waiting.length = 0;
    this.held = false;
  },
};

if (typeof HTMLImageElement !== 'undefined')
  Object.defineProperty(HTMLImageElement.prototype, 'decode', {
    configurable: true,
    writable: true,
    value(this: HTMLImageElement): Promise<void> {
      const url = this.getAttribute('src') ?? '';
      const outcome = (): Promise<void> =>
        [...broken].some((part) => url.includes(part))
          ? Promise.reject(new DOMException('The image cannot be decoded.', 'EncodingError'))
          : Promise.resolve();
      if (!globalThis.imageDecoding.held) return outcome();
      return new Promise<void>((resolve, reject) => {
        waiting.push({ url, settle: () => void outcome().then(resolve, reject) });
      });
    },
  });
