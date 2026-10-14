import { OutputTransferClient, outputTransferUrl, } from './OutputTransferClient.js';
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
        const image = await this.#read(file, accepts);
        if (!image || !accepts())
            return false;
        await this.clipboard.write(image.blob, image.selection.mimeType);
        return true;
    }
    /** Prepare exact preferred file bytes for a synchronous browser drag without retaining them here. */
    async drag(file, signal) {
        const image = await this.#read(file, () => !signal.aborted, signal);
        return image
            ? {
                file: new File([image.blob], image.selection.file.filename, {
                    type: image.selection.mimeType,
                }),
                url: image.url,
            }
            : null;
    }
    /** Share bounded validation and byte transport between Copy and outbound dragging. */
    async #read(file, accepts, signal) {
        if (!accepts())
            return null;
        const controller = new AbortController();
        const abort = () => controller.abort();
        signal?.addEventListener('abort', abort, { once: true });
        if (signal?.aborted)
            abort();
        const timer = setTimeout(() => controller.abort(), 20000);
        try {
            const selection = await this.#client.choose(file, controller.signal);
            if (!accepts() || controller.signal.aborted)
                return null;
            const url = outputTransferUrl(this.origin, selection);
            const response = await this.fetch(url, {
                signal: controller.signal,
            });
            if (!response.ok)
                throw new Error('The output image could not be read for transfer.');
            const blob = await response.blob();
            if (!accepts() || controller.signal.aborted)
                return null;
            if (blob.type !== selection.mimeType || !blob.size)
                throw new Error('The output transfer returned invalid image bytes.');
            return { blob, selection, url };
        }
        finally {
            clearTimeout(timer);
            signal?.removeEventListener('abort', abort);
        }
    }
}
