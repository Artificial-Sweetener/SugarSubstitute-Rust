import { OutputTransferClient, outputTransferUrl } from './OutputTransferClient.js';
/** Coordinate the shared file selection with browser-owned transport and clipboard boundaries. */
export class BrowserOutputTransfer {
    origin;
    fetch;
    clipboard;
    #client;
    /** Bind platform collaborators explicitly so native canvases can use the same selection endpoint. */
    constructor(origin, fetch, clipboard) {
        this.origin = origin;
        this.fetch = fetch;
        this.clipboard = clipboard;
        this.#client = new OutputTransferClient(fetch);
    }
    /** Copy exact preferred bytes only while the originating output remains current. */
    async copy(file, accepts) {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), 20000);
        try {
            const selection = await this.#client.choose(file, controller.signal);
            if (!accepts())
                return false;
            const response = await this.fetch(outputTransferUrl(this.origin, selection), {
                signal: controller.signal,
            });
            if (!response.ok)
                throw new Error('The output image could not be read for copying.');
            const blob = await response.blob();
            if (!accepts())
                return false;
            if (blob.type !== selection.mimeType || !blob.size)
                throw new Error('The output transfer returned invalid image bytes.');
            await this.clipboard.write(blob, selection.mimeType);
            return true;
        }
        finally {
            clearTimeout(timer);
        }
    }
}
