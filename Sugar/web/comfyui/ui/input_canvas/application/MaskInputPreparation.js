import { cropInputRaster } from '../domain/MaskInputEditing.js';
import { InputRasterTransformCache } from './InputRasterTransformCache.js';
import { readMaskEditSettings } from '../domain/MaskEditSettings.js';
/** Keep image and mask geometry coordinated without involving a particular canvas adapter. */
export class MaskInputPreparation {
    editing;
    #cache = new InputRasterTransformCache();
    /** Share the processing owner already used by mounted mask previews. */
    constructor(editing) {
        this.editing = editing;
    }
    /** Freeze coverage before cropping the associated image in the same source frame. */
    async prepare(image, mask, settings) {
        if (image.format !== 'rgba8' ||
            mask.format !== 'coverage8' ||
            image.width !== mask.width ||
            image.height !== mask.height)
            throw new Error('The image and mask need one common input frame for editing.');
        const accepted = readMaskEditSettings(settings);
        return this.#cache.read([image, mask], JSON.stringify(accepted), async () => {
            const edited = await this.editing.prepare(mask, accepted, 'submission');
            return Object.freeze({ image: cropInputRaster(image, edited.bounds), mask: edited });
        }, (value) => (value.image.data.length + value.mask.mask.data.length) * 2);
    }
}
