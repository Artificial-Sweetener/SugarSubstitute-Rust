import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { aspectFit, grayscaleMask } from '../domain/MaskImport.js';
/** Produce opaque grayscale rasters at exactly the surface size. */
export class BrowserMaskImport {
    document;
    /** Draw on detached canvases of the page that owns the upload. */
    constructor(document) {
        this.document = document;
    }
    /** Aspect-fit the picture onto black, then convert it to grayscale coverage. */
    async raster(file, width, height) {
        if (width * height * 4 > MAX_CANVAS_RASTER_BYTES)
            throw new Error('The mask exceeds its decoded pixel budget.');
        const bitmap = await createImageBitmap(file);
        const canvas = this.document.createElement('canvas');
        try {
            canvas.width = width;
            canvas.height = height;
            const context = canvas.getContext('2d');
            if (!context)
                throw new Error('Mask decoding is unavailable.');
            context.fillStyle = '#000';
            context.fillRect(0, 0, width, height);
            context.imageSmoothingQuality = 'high';
            const fit = aspectFit(bitmap.width, bitmap.height, width, height);
            context.drawImage(bitmap, fit.left, fit.top, fit.width, fit.height);
            const pixels = context.getImageData(0, 0, width, height).data;
            return encodeCanvasRaster(width, height, 'rgba8', grayscaleMask(pixels));
        }
        finally {
            canvas.width = canvas.height = 1;
            bitmap.close();
        }
    }
}
