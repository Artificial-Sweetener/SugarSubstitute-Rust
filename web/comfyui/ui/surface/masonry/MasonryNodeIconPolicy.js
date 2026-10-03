import { cubeFacePromptRole, findCubeFacePromptWidget } from '../CubeFacePromptPolicy.js';
import { isCubeFaceSamplerWorker } from '../CubeFaceNodeSemantics.js';
const classIcons = {
    LoadImage: 'folder',
    LoadImageMask: 'folder',
    'SimpleSyrup.LoadMaskBatch': 'folder',
    EmptyLatentImage: 'photo',
    KSampler: 'application',
    DetailerForEach: 'application',
    'SimpleSyrup.PromptEncodeStyle': 'edit',
    VectorscopeCC: 'palette',
    VAELoader: 'model',
    CheckpointLoaderSimple: 'model',
};
const modelClasses = new Set([
    'UpscaleModelLoader',
    'UltralyticsDetectorProvider',
    'SimpleSyrup.LoadUltralyticsModel',
]);
/** Prefer prompt/class semantics and sampler contracts before a model-picker fallback. */
export function resolveMasonryNodeIcon(node) {
    if (findCubeFacePromptWidget(node))
        return cubeFacePromptRole(node) === 'negative' ? 'eraser' : 'edit';
    const classType = (node.class_type ?? node.type ?? '').trim();
    const explicit = Object.hasOwn(classIcons, classType) ? classIcons[classType] : undefined;
    if (explicit)
        return explicit;
    if (isCubeFaceSamplerWorker(node))
        return 'application';
    if ((node.widgets ?? []).some((widget) => {
        if (widget.type !== 'combo' && !Array.isArray(widget.options?.values))
            return false;
        const name = widget.name.trim().toLowerCase();
        return (name === 'vae_name' ||
            (name === 'vae' && classType.startsWith('SimpleSyrup.')) ||
            (name === 'model_name' && modelClasses.has(classType)) ||
            /checkpoint|ckpt|lora|unet|diffusion|control_?net|clip|text_encoder/.test(name));
    }))
        return 'model';
    return null;
}
