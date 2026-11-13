import { encodeCanvasRaster, MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
import { grayscaleCoverage } from '../domain/MaskImport.js';
/** Produce coverage rasters at exactly the surface size. */
export class BrowserMaskImport {
    document;
    /** Draw on detached canvases of the page that owns the upload. */
    constructor(document) {
        this.document = document;
    }
    /**
     * Stretch the picture to the surface, ignoring its aspect ratio, with smooth resampling, then
     * read its gray levels as coverage.
     */
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
            context.imageSmoothingEnabled = true;
            context.imageSmoothingQuality = 'high';
            context.drawImage(bitmap, 0, 0, width, height);
            const pixels = context.getImageData(0, 0, width, height).data;
            return encodeCanvasRaster(width, height, 'coverage8', grayscaleCoverage(pixels));
        }
        finally {
            canvas.width = canvas.height = 1;
            bitmap.close();
        }
    }
}
