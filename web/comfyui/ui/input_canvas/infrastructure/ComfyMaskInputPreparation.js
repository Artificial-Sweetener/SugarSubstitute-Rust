import { preparedSourceFile, } from '../application/InputSourcePreparation.js';
import { nativeMaskSource, maskSourceImage } from '../domain/MaskSourcePixels.js';
import { DEFAULT_MASK_EDITS, hasMaskEdits } from '../domain/MaskEditSettings.js';
import { sameCanvasSourceDocument } from '../../core/embedded/CanvasRuntime.js';
/** Own native decoding and immutable upload translation while the application owns coordinated pixel processing. */
export class ComfyMaskInputPreparation {
    rasters;
    assets;
    pixels;
    pairs;
    /** Reuse source codecs, uploads and graph relationships already used by normal source preparation. */
    constructor(rasters, assets, pixels, pairs) {
        this.rasters = rasters;
        this.assets = assets;
        this.pixels = pixels;
        this.pairs = pairs;
    }
    /** Prepare each eligible scalar pair once, preserving image-node operations and custom mask interpretation. */
    async prepare(targets, sources, pairs, ports, fallbackFrame) {
        const sourceFiles = new Map(), returnInputs = new Map();
        const replacements = [];
        const byKey = new Map(targets.map((target) => [target.key, target]));
        for (const [maskKey, imageKey] of pairs) {
            const mask = byKey.get(maskKey), image = byKey.get(imageKey);
            const settings = mask?.binding.edits ?? DEFAULT_MASK_EDITS;
            if (!mask || !image || !hasMaskEdits(settings))
                continue;
            if (mask.loader.kind !== 'mask' ||
                image.loader.kind !== 'image' ||
                mask.loader.ordered ||
                image.loader.ordered ||
                mask.binding.entries.length !== 1 ||
                image.binding.entries.length !== 1 ||
                image.maskOutput)
                throw new Error('Mask editing requires one image and one mask.');
            if (mask.unresolvedSettings.length)
                throw new Error('Mask editing requires captured loader settings.');
            const maskEntry = mask.binding.entries[0], imageEntry = image.binding.entries[0];
            if (!maskEntry || !imageEntry)
                throw new Error('The edited input source is unavailable.');
            const maskFile = preparedSourceFile(mask.key, maskEntry, sources), imageFile = preparedSourceFile(image.key, imageEntry, sources);
            const maskSnapshot = sources.snapshots.get(JSON.stringify([mask.key, maskEntry.id]));
            const imageSnapshot = sources.snapshots.get(JSON.stringify([image.key, imageEntry.id]));
            if (maskSnapshot &&
                imageSnapshot &&
                (maskEntry.source.kind !== 'canvas' ||
                    imageEntry.source.kind !== 'canvas' ||
                    maskEntry.source.canvasId !== imageEntry.source.canvasId ||
                    !sameCanvasSourceDocument(maskSnapshot.document, imageSnapshot.document)))
                throw new Error('The edited image and mask belong to different canvas documents.');
            const coverage = (maskSnapshot
                ? nativeMaskSource(maskSnapshot.pixels, mask.loader, mask.settings, false)
                : null) ?? (await this.rasters.load(maskFile, mask.settings, 0));
            const originalImage = imageSnapshot?.pixels ?? (await this.rasters.load(imageFile, image.settings, 0));
            const frame = imageSnapshot?.document ?? maskSnapshot?.document ?? fallbackFrame;
            if (!frame || originalImage.width !== frame.width || originalImage.height !== frame.height)
                throw new Error('The edited inputs have no declared mapping to the canvas frame.');
            const prepared = await this.pixels.prepare(originalImage, coverage, settings);
            const croppedFile = prepared.image === originalImage ? imageFile : await this.assets.stage(prepared.image);
            const editedFile = await this.assets.stage(maskSourceImage(prepared.mask.mask));
            sourceFiles.set(image.key, croppedFile);
            returnInputs.set(image.key, {
                file: croppedFile,
                classType: image.loader.classType,
                field: image.loader.field,
                pixels: prepared.image,
                mapping: prepared.mask.mapping,
                sourceSize: { width: originalImage.width, height: originalImage.height },
            });
            returnInputs.set(mask.key, {
                file: editedFile,
                classType: 'LoadImageMask',
                field: 'image',
                pixels: prepared.mask.mask,
                mapping: prepared.mask.mapping,
                sourceSize: { width: originalImage.width, height: originalImage.height },
            });
            const outputs = ports.get(mask.loader.classType)?.outputs ?? [];
            const slots = outputs.flatMap((kind, slot) => (kind === 'mask' ? [slot] : []));
            if (slots.length !== 1 || slots[0] === undefined)
                throw new Error('The mask loader has ambiguous outputs.');
            replacements.push({
                nodePath: mask.nodePath,
                classType: mask.loader.classType,
                field: mask.loader.field,
                file: maskFile,
                slot: slots[0],
                kind: 'mask',
                preparedFile: editedFile,
            });
        }
        return { sourceFiles, returnInputs, replacements: Object.freeze(replacements) };
    }
}
