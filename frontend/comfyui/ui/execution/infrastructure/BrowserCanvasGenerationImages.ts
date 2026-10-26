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
/** Prepare full-resolution generation bytes without depending on the web canvas or its resources. */
import {
  MAX_CANVAS_IMAGE_BYTES,
  type CanvasCancellationSource,
} from '../../core/embedded/CanvasRuntime.js';
import type { CanvasGenerationImages } from '../domain/CanvasGenerationImages.js';
import type { OutputFileReference } from '../domain/WorkflowOutput.js';
import { OutputMediaClient } from '../../output_saving/OutputMediaClient.js';
import type { CanvasImageRegion } from '../../../../../integrations/contracts/CanvasImageRegion.js';
import { browserCanvasImage } from './BrowserCanvasImage.js';

/** Reuse the existing image rendition endpoint and bridge portable cancellation into HTTP transport. */
export class BrowserCanvasGenerationImages implements CanvasGenerationImages {
  readonly #media: OutputMediaClient;
  /** Bind the same-origin transport without acquiring output-viewer state. */
  constructor(origin: string, fetch: typeof globalThis.fetch) {
    this.#media = new OutputMediaClient(origin, fetch);
  }
  /** Encode intrinsic PNG bytes and reject oversized transfers before native decoding. */
  async load(
    file: OutputFileReference,
    cancellation: CanvasCancellationSource,
    region: CanvasImageRegion | null,
  ) {
    const controller = new AbortController();
    const stop = cancellation.subscribe(() => controller.abort());
    try {
      const media = await this.#media.image(file, 'png', controller.signal);
      if (media.blob.size > MAX_CANVAS_IMAGE_BYTES)
        throw new Error('This image exceeds the canvas transfer byte budget.');
      return await browserCanvasImage(media.blob, region, cancellation);
    } finally {
      stop();
    }
  }
}
