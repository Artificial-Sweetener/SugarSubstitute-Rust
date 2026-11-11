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
/** Stage reusable source images and selected files through Comfy's normal immutable input upload boundary. */
import type { CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { isRecord } from '../../types/common.js';
import { encodeSourcePng } from '../../core/embedded/CanvasPngEncoding.js';
import { InputRasterTransformCache } from '../application/InputRasterTransformCache.js';

/** Own one upload policy shared by source controls, migration and queue-time canvas capture. */
export class BrowserCanvasSourceAssets {
  readonly #verified = new Set<string>();
  readonly #staged = new InputRasterTransformCache<string>();
  /** Inject same-origin transport without giving application code browser networking APIs. */
  constructor(private readonly fetch: (input: string, init: RequestInit) => Promise<Response>) {}

  /** Reuse an immutable upload when source pixels survive capture or generation boundaries unchanged. */
  stage(pixels: CanvasPixelRaster): Promise<string> {
    return this.#staged.read(
      [pixels],
      '',
      async () =>
        await this.#upload(await encodeSourcePng(pixels), `source-${crypto.randomUUID()}.png`),
      (file) => file.length * 2,
    );
  }

  /** Upload the user's selected file without decoding or changing its original channel data. */
  async file(file: File): Promise<string> {
    return await this.#upload(file, file.name);
  }
  /** Recognize real uploads without inventing choices for the native file widget. */
  verified(file: string): boolean {
    return this.#verified.has(file);
  }
  /** Confirm restored source files through Comfy's media boundary before publishing native choices. */
  async verify(file: string): Promise<boolean> {
    if (this.#verified.has(file)) return true;
    const match = /^(.*?)(?: \[(input|output|temp)\])?$/.exec(file);
    const name = (match?.[1] ?? file).replaceAll('\\', '/'),
      parts = name.split('/'),
      filename = parts.pop() ?? '';
    const response = await this.fetch(
      '/view?' +
        new URLSearchParams({
          filename,
          subfolder: parts.join('/'),
          type: match?.[2] ?? 'input',
        }).toString(),
      { method: 'HEAD', signal: AbortSignal.timeout(30000) },
    );
    if (response.ok) this.#verified.add(file);
    return response.ok;
  }

  /** Validate server-chosen relative input identity before allowing it into persisted source state. */
  async #upload(blob: Blob, name: string): Promise<string> {
    const form = new FormData();
    form.append('image', blob, name);
    form.append('type', 'input');
    form.append('subfolder', 'sugarcubes/sources');
    form.append('overwrite', 'false');
    const response = await this.fetch('/upload/image', {
      method: 'POST',
      body: form,
      signal: AbortSignal.timeout(30000),
    });
    const result: unknown = await response.json();
    if (
      !response.ok ||
      !isRecord(result) ||
      typeof result.name !== 'string' ||
      !result.name ||
      /[\\/:]|\p{Cc}/u.test(result.name) ||
      result.name === '.' ||
      result.name === '..' ||
      result.subfolder !== 'sugarcubes/sources' ||
      result.type !== 'input'
    )
      throw new Error('Comfy could not upload the source image.');
    const file = `${result.subfolder}/${result.name}`;
    this.#verified.add(file);
    return file;
  }
}
