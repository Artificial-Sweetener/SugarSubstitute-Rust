import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';
import { isRecord } from '../../types/common.js';
/** Keep native interpretation and bounded PNG decoding shared by all input consumers. */
export class BrowserInputRasters {
    document;
    request;
    /** Bind the owning document and same-origin HTTP boundary explicitly. */
    constructor(document, request) {
        this.document = document;
        this.request = request;
    }
    /** Decode native output at full resolution only when explicitly requested by execution preparation. */
    async load(file, settings, maximumSize) {
        if (!Number.isSafeInteger(maximumSize) ||
            maximumSize < 0 ||
            maximumSize > INPUT_PREVIEW_MAXIMUM)
            throw new Error('Invalid input raster size.');
        const response = await this.request('/sugarcubes/v2/canvas-inputs/preview', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ file, settings, maximumSize }),
            signal: AbortSignal.timeout(30000),
        });
        if (!response.ok) {
            const error = await response.json();
            throw new Error(isRecord(error) && typeof error.error === 'string'
                ? error.error.slice(0, 512)
                : isRecord(error) && isRecord(error.error) && typeof error.error.message === 'string'
                    ? error.error.message.slice(0, 512)
                    : 'The input source is unavailable.');
        }
        const blob = await response.blob();
        if (blob.type !== 'image/png' || blob.size > (maximumSize ? 8 : 64) * 1024 * 1024)
            throw new Error('The source has an invalid image payload.');
        const bitmap = await createImageBitmap(blob);
        try {
            if (bitmap.width < 1 ||
                bitmap.height < 1 ||
                bitmap.width * bitmap.height * 4 > MAX_CANVAS_RASTER_BYTES ||
                (maximumSize > 0 && (bitmap.width > maximumSize || bitmap.height > maximumSize)))
                throw new Error('The source exceeds its decoded pixel budget.');
            const canvas = this.document.createElement('canvas');
            try {
                canvas.width = bitmap.width;
                canvas.height = bitmap.height;
                const context = canvas.getContext('2d');
                if (!context)
                    throw new Error('Source pixel decoding is unavailable.');
                context.drawImage(bitmap, 0, 0);
                const data = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
                if (settings.kind === 'image')
                    return encodeCanvasRaster(bitmap.width, bitmap.height, 'rgba8', new Uint8Array(data));
                const coverage = new Uint8Array(bitmap.width * bitmap.height);
                for (let index = 0; index < coverage.length; index++)
                    coverage[index] = data[index * 4] ?? 0;
                return encodeCanvasRaster(bitmap.width, bitmap.height, 'coverage8', coverage);
            }
            finally {
                canvas.width = canvas.height = 1;
            }
        }
        finally {
            bitmap.close();
        }
    }
}
