import { CanvasSourceCapture } from '../application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../application/CanvasSourcePreviews.js';
import { InputSourcePreparation } from '../application/InputSourcePreparation.js';
import { BrowserCanvasSourceAssets } from './BrowserCanvasSourceAssets.js';
import { LegacyMaskMigration } from './LegacyMaskMigration.js';
import { ComfyInputSourceDefaults } from './ComfyInputSourceDefaults.js';
import { ComfyInputLoaders } from './ComfyInputLoaders.js';
import { ComfyInputPreparation } from './ComfyInputPreparation.js';
import { MasonryInputHost } from './MasonryInputHost.js';
import { ComfyInputBindings } from './ComfyInputBindings.js';
import { BrowserInputPreviews } from './BrowserInputPreviews.js';
import { BrowserInputRasters } from './BrowserInputRasters.js';
import { BrowserMaskEditing } from './BrowserMaskEditing.js';
import { MaskInputPreparation } from '../application/MaskInputPreparation.js';
import { ComfyMaskInputPreparation } from './ComfyMaskInputPreparation.js';
import { PreparedInputPromptClient } from './PreparedInputPromptClient.js';
import { ComfyInputReturnPreparation } from './ComfyInputReturnPreparation.js';
import { ComfyInputAssociations } from './ComfyInputAssociations.js';
/** Keep definition registration, execution preparation and view lifetime under explicit owners. */
export function createCanvasInputs(options) {
    const { document, host, legacyHost, report } = options;
    const loaders = new ComfyInputLoaders();
    const assets = new BrowserCanvasSourceAssets((input, init) => fetch(input, init));
    const capture = new CanvasSourceCapture(host);
    const rasters = new BrowserInputRasters(document, (input, init) => fetch(input, init));
    const maskEditing = new BrowserMaskEditing();
    const previews = new CanvasSourcePreviews(capture, host, report);
    const legacy = new LegacyMaskMigration(legacyHost, assets);
    const defaults = new ComfyInputSourceDefaults(host, legacy);
    const bindings = new ComfyInputBindings(loaders, {
        host,
        editable: options.editable,
        files: new BrowserInputPreviews((source) => assets.stage(source), rasters),
        maskEditing,
        legacy,
        verifyFile: (file) => assets.verify(file),
        verifiedFile: (file) => assets.verified(file),
        report,
    });
    const associations = new ComfyInputAssociations(loaders, bindings, options.graph, report, options.logger);
    const masonry = new MasonryInputHost(document, loaders, previews, (file) => assets.file(file), report, bindings, associations);
    document.defaultView?.addEventListener('pagehide', () => {
        associations.dispose();
        bindings.clear();
        maskEditing.dispose();
    }, { once: true });
    return {
        loaders,
        preparation: new ComfyInputPreparation(loaders, new InputSourcePreparation(capture, assets), defaults, new ComfyInputReturnPreparation(rasters, async () => {
            const source = host();
            if (!source)
                return null;
            const context = await capture.context(source.canvasId);
            return context.document ? { canvasId: source.canvasId, document: context.document } : null;
        }), new ComfyMaskInputPreparation(rasters, assets, new MaskInputPreparation(maskEditing), () => associations.preparationPairs()), new PreparedInputPromptClient((input, init) => fetch(input, init))),
        masonry,
        bindings,
        associations,
        define: (definitions) => {
            loaders.define(definitions);
            associations.definitionsChanged();
        },
        setup: () => {
            associations.start();
            masonry.setup();
        },
        beforeConfigureGraph: () => {
            masonry.beforeConfigureGraph();
            associations.beforeConfigureGraph();
            bindings.clear();
        },
        afterConfigureGraph: async () => {
            associations.start();
            await bindings.ready();
        },
    };
}
