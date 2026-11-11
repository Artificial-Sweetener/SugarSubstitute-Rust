import { retainedOutputCapabilities } from '../../output_saving/BrowserRetainedOutputTransfer.js';
import { retainedCanvasOutput } from './RetainedCanvasOutput.js';
/** Preserve existing backend actions while accurately limiting immutable retained-only originals. */
export function canvasOutputTransfers(transfer) {
    return {
        copy: (item, accepts, format, signal) => transfer.copy(item.file, accepts, format, signal, retainedCanvasOutput(item)),
        fileAction: (item, accepts, signal) => transfer.fileAction(item.file, accepts, signal, item.mediaKind === 'image' ? 'image' : 'media', retainedCanvasOutput(item)),
        mediaProfile: (item) => transfer.profile(retainedCanvasOutput(item)),
        mediaCapabilities: (item) => {
            if (item.unavailable)
                return { copy: false, file: false, photoshop: false, drag: false };
            const source = retainedCanvasOutput(item);
            if (source) {
                const actions = retainedOutputCapabilities(source);
                return {
                    copy: actions.copy,
                    file: actions.download,
                    photoshop: actions.photoshop,
                    drag: actions.drag,
                };
            }
            return {
                copy: item.mediaKind === 'image',
                file: true,
                photoshop: transfer.profile().photoshop,
                drag: item.mediaKind === 'image',
            };
        },
        photoshop: (document, layers, accepts, signal) => {
            if (layers.some((layer) => layer.item.unavailable))
                return Promise.resolve(false);
            return transfer.photoshop(document, layers.map((layer) => {
                const retained = retainedCanvasOutput(layer.item);
                return { file: layer.item.file, name: layer.name, ...(retained ? { retained } : {}) };
            }), accepts, signal);
        },
        drag: (item, signal) => {
            if (item.unavailable)
                return Promise.resolve(null);
            const source = retainedCanvasOutput(item);
            if (source && !retainedOutputCapabilities(source).drag)
                return Promise.resolve(null);
            return transfer.drag(item.file, signal, source);
        },
    };
}
