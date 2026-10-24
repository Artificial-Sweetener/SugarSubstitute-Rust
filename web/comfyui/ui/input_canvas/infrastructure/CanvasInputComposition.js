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
/** Keep definition registration, execution preparation and view lifetime under explicit owners. */
export function createCanvasInputs(document, host, legacyHost, report) {
    const loaders = new ComfyInputLoaders();
    const assets = new BrowserCanvasSourceAssets((input, init) => fetch(input, init));
    const capture = new CanvasSourceCapture(host);
    const previews = new CanvasSourcePreviews(capture, host, report);
    const legacy = new LegacyMaskMigration(legacyHost, assets);
    const defaults = new ComfyInputSourceDefaults(host, legacy);
    const bindings = new ComfyInputBindings(loaders, {
        host,
        files: new BrowserInputPreviews(document, (source) => assets.stage(source)),
        legacy,
        verifyFile: (file) => assets.verify(file),
        verifiedFile: (file) => assets.verified(file),
        report,
    });
    const masonry = new MasonryInputHost(document, loaders, previews, (file) => assets.file(file), report, bindings);
    document.defaultView?.addEventListener('pagehide', () => bindings.clear(), { once: true });
    return {
        loaders,
        preparation: new ComfyInputPreparation(loaders, new InputSourcePreparation(capture, assets), defaults),
        masonry,
        bindings,
        beforeConfigureGraph: () => {
            masonry.beforeConfigureGraph();
            bindings.clear();
        },
    };
}
