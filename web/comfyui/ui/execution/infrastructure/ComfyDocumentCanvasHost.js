import { documentCanvasAdapter } from './DocumentCanvasAdapter.js';
import { BrowserCanvasGenerationImages } from './BrowserCanvasGenerationImages.js';
/** Own the page registration and feedback without a separate execution-delivery subscription. */
export class ComfyDocumentCanvasHost {
    canvas;
    window;
    #unregister;
    /** Translate document-host primitives into the same capability contract as other canvases. */
    constructor(canvases, canvas, window, logger, notify) {
        this.canvas = canvas;
        this.window = window;
        this.#unregister = canvases.register(documentCanvasAdapter('embedded-document', canvas, new BrowserCanvasGenerationImages(window.location.origin, (input, init) => window.fetch(input, init)), (error) => {
            logger.error('SugarCubes document canvas operation failed.', { error });
            notify(error instanceof Error ? error.message : String(error));
        }));
        window.addEventListener('pagehide', this.dispose);
    }
    /** Select this destination only while the authenticated document host advertises insertion. */
    get available() {
        return this.canvas.readCanvas()?.capabilities.insertImageLayer ?? false;
    }
    /** Retire this adapter while leaving other canvases registered. */
    dispose = () => {
        this.window.removeEventListener('pagehide', this.dispose);
        this.#unregister();
    };
}
