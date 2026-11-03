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
import { createWorkflowSurfaces } from './WorkflowSurfaceComposition.js';
/** Keep definition registration, execution preparation and view lifetime under explicit owners. */
export function createCanvasInputs(options) {
    const { document, host, legacyHost, report } = options;
    const loaders = new ComfyInputLoaders();
    const assets = new BrowserCanvasSourceAssets((input, init) => fetch(input, init));
    const capture = new CanvasSourceCapture(host, (canvasId, documentId) => surfaces.provisioning.document(canvasId, documentId));
    const rasters = new BrowserInputRasters(document, (input, init) => fetch(input, init));
    const maskEditing = new BrowserMaskEditing();
    const previews = new CanvasSourcePreviews(capture, host, report);
    const legacy = new LegacyMaskMigration(legacyHost, assets);
    const defaults = new ComfyInputSourceDefaults(host, legacy, (id) => options.canvases.provisions(id));
    const bindings = new ComfyInputBindings(loaders, {
        host,
        canvases: options.canvases,
        refreshPreviews: (sources) => previews.refreshSources(sources),
        files: new BrowserInputPreviews((source) => assets.stage(source), rasters, options.memory),
        maskEditing,
        legacy,
        verifyFile: (file) => assets.verify(file),
        verifiedFile: (file) => assets.verified(file),
        report,
    });
    const associations = new ComfyInputAssociations(loaders, bindings, options.graph, report, options.logger);
    const surfaces = createWorkflowSurfaces({
        document,
        associations,
        bindings,
        loaders,
        assets,
        graph: options.graph,
        workflowHost: options.workflowHost,
        accent: options.accent,
        notice: options.notice,
        report,
    });
    const masonry = new MasonryInputHost(document, loaders, previews, (file) => assets.file(file), report, bindings, associations, surfaces.cards);
    const canvasReady = () => {
        void surfaces.provisioning.refresh();
    };
    document.addEventListener('sugarcubes-canvas-ready', canvasReady);
    document.defaultView?.addEventListener('pagehide', () => {
        document.removeEventListener('sugarcubes-canvas-ready', canvasReady);
        surfaces.dispose();
        associations.dispose();
        bindings.clear();
        maskEditing.dispose();
    }, { once: true });
    const preparation = new ComfyInputPreparation(loaders, new InputSourcePreparation(capture, assets), defaults, new ComfyInputReturnPreparation(rasters, async () => {
        const source = host();
        if (!source)
            return null;
        const context = await capture.context(source.canvasId);
        return context.document ? { canvasId: source.canvasId, document: context.document } : null;
    }), new ComfyMaskInputPreparation(rasters, assets, new MaskInputPreparation(maskEditing), () => associations.preparationPairs()), new PreparedInputPromptClient((input, init) => fetch(input, init)));
    return {
        loaders,
        preparation: {
            prepare: async (payload) => {
                await surfaces.provisioning.assertGenerationReady();
                return await preparation.prepare(payload);
            },
        },
        masonry,
        bindings,
        associations,
        surfaces,
        define: (definitions) => {
            loaders.define(definitions);
            associations.definitionsChanged();
        },
        setup: () => {
            associations.start();
            masonry.setup();
            void surfaces.provisioning.refresh();
        },
        beforeConfigureGraph: () => {
            masonry.beforeConfigureGraph();
            associations.beforeConfigureGraph();
            bindings.clear();
        },
        afterConfigureGraph: async () => {
            associations.start();
            void surfaces.provisioning.refresh();
            await bindings.ready();
        },
    };
}
