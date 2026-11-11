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
/** Decode bounded native loader output without owning preview caching, source staging, or canvas state. */
import type { InputRasterPort } from '../application/InputRasterPort.js';
import type { InputPreviewSettings } from '../domain/InputLoader.js';
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';
import { isRecord } from '../../types/common.js';

/** Keep native interpretation and bounded PNG decoding shared by all input consumers. */
export class BrowserInputRasters implements InputRasterPort {
  /** Bind the owning document and same-origin HTTP boundary explicitly. */
  constructor(
    private readonly document: Document,
    private readonly request: (input: string, init: RequestInit) => Promise<Response>,
  ) {}

  /** Decode native output at full resolution only when explicitly requested by execution preparation. */
  async load(
    file: string,
    settings: InputPreviewSettings,
    maximumSize: number,
  ): Promise<CanvasPixelRaster> {
    if (
      !Number.isSafeInteger(maximumSize) ||
      maximumSize < 0 ||
      maximumSize > INPUT_PREVIEW_MAXIMUM
    )
      throw new Error('Invalid input raster size.');
    const response = await this.request('/sugarcubes/v2/canvas-inputs/preview', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ file, settings, maximumSize }),
      signal: AbortSignal.timeout(30000),
    });
    if (!response.ok) {
      const error: unknown = await response.json();
      throw new Error(
        isRecord(error) && typeof error.error === 'string'
          ? error.error.slice(0, 512)
          : isRecord(error) && isRecord(error.error) && typeof error.error.message === 'string'
            ? error.error.message.slice(0, 512)
            : 'The input source is unavailable.',
      );
    }
    const blob = await response.blob();
    if (blob.type !== 'image/png' || blob.size > (maximumSize ? 8 : 64) * 1024 * 1024)
      throw new Error('The source has an invalid image payload.');
    const bitmap = await createImageBitmap(blob);
    try {
      if (
        bitmap.width < 1 ||
        bitmap.height < 1 ||
        bitmap.width * bitmap.height * 4 > MAX_CANVAS_RASTER_BYTES ||
        (maximumSize > 0 && (bitmap.width > maximumSize || bitmap.height > maximumSize))
      )
        throw new Error('The source exceeds its decoded pixel budget.');
      const canvas = this.document.createElement('canvas');
      try {
        canvas.width = bitmap.width;
        canvas.height = bitmap.height;
        const context = canvas.getContext('2d');
        if (!context) throw new Error('Source pixel decoding is unavailable.');
        context.drawImage(bitmap, 0, 0);
        const data = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
        if (settings.kind === 'image')
          return encodeCanvasRaster(bitmap.width, bitmap.height, 'rgba8', new Uint8Array(data));
        const coverage = new Uint8Array(bitmap.width * bitmap.height);
        for (let index = 0; index < coverage.length; index++)
          coverage[index] = data[index * 4] ?? 0;
        return encodeCanvasRaster(bitmap.width, bitmap.height, 'coverage8', coverage);
      } finally {
        canvas.width = canvas.height = 1;
      }
    } finally {
      bitmap.close();
    }
  }
}
