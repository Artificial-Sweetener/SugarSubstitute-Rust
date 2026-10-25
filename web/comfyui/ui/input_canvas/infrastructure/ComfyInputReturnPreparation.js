import { usesNativeMaskChannels, nativeMaskOutputChannel } from '../domain/InputLoader.js';
import { interpretMaskSource } from '../domain/MaskSourcePixels.js';
import { compiledInputReturnAssociations, } from './ComfyInputReturnGraph.js';
import { MAX_CANVAS_RASTER_BYTES, sameCanvasSourceDocument, } from '../../core/embedded/CanvasRuntime.js';
/** Retain an immutable preparation until the authoritative compiler has supplied its final graph. */
class PreparedCanvasReturns {
    inputs;
    ports;
    fallback;
    /** Bind captured field identities and frozen node asset declarations, never live host handles. */
    constructor(inputs, ports, fallback) {
        this.inputs = inputs;
        this.ports = ports;
        this.fallback = fallback;
    }
    /** Attribute only outputs with one compatible captured image and return mask. */
    resolve(prompt) {
        const fields = new Map(this.inputs.map((input) => [input.key, input]));
        return Object.freeze(compiledInputReturnAssociations(prompt, this.inputs, this.ports).map((association) => {
            if (association.kind === 'unavailable')
                return {
                    kind: 'unavailable',
                    executionPath: association.output,
                    message: association.message,
                };
            const mask = fields.get(association.mask), image = fields.get(association.image);
            const origin = image?.origin ?? mask?.origin ?? this.fallback;
            if (mask?.unresolvedSettings.length)
                return {
                    kind: 'unavailable',
                    executionPath: association.output,
                    message: 'The inpaint mask uses graph-connected settings without a captured interpretation.',
                };
            if (!mask || !image || !origin || !mask.masks.length)
                return {
                    kind: 'unavailable',
                    executionPath: association.output,
                    message: 'The inpaint inputs have no captured canvas placement.',
                };
            if (image.origin &&
                mask.origin &&
                (image.origin.canvasId !== mask.origin.canvasId ||
                    !sameCanvasSourceDocument(image.origin.document, mask.origin.document)))
                return {
                    kind: 'unavailable',
                    executionPath: association.output,
                    message: 'The inpaint image and mask belong to different canvas documents.',
                };
            const { width, height } = origin.document;
            if (mask.masks.some((pixels) => pixels.width !== width || pixels.height !== height))
                return {
                    kind: 'unavailable',
                    executionPath: association.output,
                    message: 'The inpaint mask has no declared mapping to the captured canvas frame.',
                };
            const mapping = Object.freeze({
                imageSize: Object.freeze({ width, height }),
                imageBounds: Object.freeze({ left: 0, top: 0, width, height }),
                documentBounds: Object.freeze({ left: 0, top: 0, width, height }),
            });
            return Object.freeze({
                kind: 'region',
                executionPath: association.output,
                canvasId: origin.canvasId,
                document: origin.document,
                regions: Object.freeze(mask.masks.map((pixels) => Object.freeze({
                    mapping,
                    mask: pixels,
                    coverage: association.composited ? 'support' : 'mask',
                }))),
            });
        }));
    }
}
/** Decode masks through the same authoritative conventions used by input previews. */
export class ComfyInputReturnPreparation {
    rasters;
    context;
    /** Inject native source decoding and optional canvas context without identifying Photoshop. */
    constructor(rasters, context) {
        this.rasters = rasters;
        this.context = context;
    }
    /** Capture a file-only workflow's destination before decoding or uploading can await. */
    async origin(targets) {
        if (!targets.some((target) => target.loader.kind === 'mask' || target.maskOutput) ||
            targets.some((target) => target.binding.entries.some((entry) => entry.source.kind === 'canvas')))
            return null;
        const context = await this.context();
        return context
            ? Object.freeze({
                canvasId: context.canvasId,
                document: Object.freeze({ ...context.document }),
            })
            : null;
    }
    /** Freeze all loader settings and effective coverage before accepting a queue request. */
    async prepare(targets, prepared, ports, fallback) {
        if (!targets.some((target) => target.loader.kind === 'mask' || target.maskOutput))
            return undefined;
        if (!fallback && !prepared.snapshots.size)
            return undefined;
        const inputs = [];
        let retained = 0;
        for (const target of targets) {
            const files = [], masks = [];
            let origin = null;
            for (const entry of target.binding.entries) {
                const key = JSON.stringify([target.key, entry.id]);
                const snapshot = prepared.snapshots.get(key);
                const file = entry.source.kind === 'canvas' ? prepared.files.get(key) : entry.source.file;
                if (!file)
                    throw new Error('The captured inpaint source is unavailable.');
                files.push(file);
                if (snapshot && entry.source.kind === 'canvas') {
                    const current = { canvasId: entry.source.canvasId, document: snapshot.document };
                    if (origin &&
                        (origin.canvasId !== current.canvasId ||
                            !sameCanvasSourceDocument(origin.document, current.document)))
                        throw new Error('One inpaint source collection must belong to one canvas document.');
                    origin = Object.freeze(current);
                }
                if ((target.loader.kind === 'mask' || target.maskOutput) &&
                    !target.unresolvedSettings.length) {
                    const channel = target.maskOutput
                        ? nativeMaskOutputChannel(target.loader)
                        : target.settings.inputs.channel;
                    const pixels = snapshot &&
                        (target.maskOutput || usesNativeMaskChannels(target.loader)) &&
                        typeof channel === 'string'
                        ? interpretMaskSource(snapshot.pixels, channel)
                        : await this.rasters.load(file, { ...target.settings, kind: 'mask' }, 0);
                    if (pixels.format !== 'coverage8')
                        throw new Error('The inpaint loader did not return mask coverage.');
                    retained += pixels.width * pixels.height;
                    if (retained > MAX_CANVAS_RASTER_BYTES)
                        throw new Error('The inpaint mask collection exceeds its retained pixel budget.');
                    masks.push(Object.freeze({ ...pixels }));
                }
            }
            const field = Object.freeze({
                key: target.key,
                nodePath: target.nodePath,
                classType: target.loader.classType,
                field: target.loader.field,
                kind: target.loader.kind,
                files: Object.freeze(files),
                masks: Object.freeze(target.loader.kind === 'mask' ? masks : []),
                unresolvedSettings: target.unresolvedSettings,
                origin,
            });
            inputs.push(field);
            if (target.maskOutput)
                inputs.push(Object.freeze({
                    ...field,
                    key: JSON.stringify([target.key, 'mask-output']),
                    kind: 'mask',
                    masks: Object.freeze(masks),
                }));
        }
        if (!fallback && !inputs.some((input) => input.origin))
            return undefined;
        return new PreparedCanvasReturns(Object.freeze(inputs), ports, fallback);
    }
}
