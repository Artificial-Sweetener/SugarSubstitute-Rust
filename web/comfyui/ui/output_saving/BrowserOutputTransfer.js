import { OutputMediaClient } from './OutputMediaClient.js';
import { OutputMediaProfileSource } from './OutputMediaProfileSource.js';
import { OutputTransferClient, outputFileUrl, } from './OutputTransferClient.js';
/** Coordinate the shared file selection with browser-owned transport and clipboard boundaries. */
export class BrowserOutputTransfer {
    origin;
    fetch;
    clipboard;
    download;
    #client;
    #media;
    #profile;
    #disposed = false;
    /** Bind platform collaborators explicitly so native canvases can use the same selection endpoint. */
    constructor(origin, fetch, clipboard, download) {
        this.origin = origin;
        this.fetch = fetch;
        this.clipboard = clipboard;
        this.download = download;
        this.#client = new OutputTransferClient(fetch);
        this.#media = new OutputMediaClient(origin, fetch);
        this.#profile = new OutputMediaProfileSource(() => this.#media.profile());
    }
    /** Expose a conservative cached profile without network work when a context menu opens. */
    profile() {
        return this.#profile.read();
    }
    /** Observe accepted profile changes independently of canvas or image resource identity. */
    subscribe(listener) {
        return this.#profile.subscribe(listener);
    }
    /** Refresh only on setup or accepted preference changes, refusing superseded responses. */
    refresh() {
        return this.#profile.refresh();
    }
    /** Copy exact preferred bytes only while the originating output remains current. */
    async copy(file, accepts, format, signal) {
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        const image = await this.#media.image(file, format, signal);
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        await this.clipboard.write(image.blob, image.mimeType);
        return true;
    }
    /**
     * Reveal through the local backend or start the browser download without rendering another image.
     *
     * Only still images have preferred renditions, so other media download their exact file.
     */
    async fileAction(file, accepts, signal, content = 'image') {
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        if (this.#profile.read().fileAction === 'reveal')
            await this.#media.reveal(file, signal);
        else
            this.download.start(content === 'image' ? this.#media.downloadUrl(file) : outputFileUrl(this.origin, file));
        return true;
    }
    /** Retire profile listeners and refuse late clipboard writes after the browser host is torn down. */
    dispose() {
        this.#disposed = true;
        this.#profile.dispose();
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
        if (this.#disposed || !accepts())
            return null;
        const controller = new AbortController();
        const abort = () => controller.abort();
        signal?.addEventListener('abort', abort, { once: true });
        if (signal?.aborted)
            abort();
        const timer = setTimeout(() => controller.abort(), 20000);
        try {
            const selection = await this.#client.choose(file, controller.signal);
            if (this.#disposed || !accepts() || controller.signal.aborted)
                return null;
            const url = outputFileUrl(this.origin, selection.file);
            const response = await this.fetch(url, {
                signal: controller.signal,
            });
            if (!response.ok)
                throw new Error('The output image could not be read for transfer.');
            const blob = await response.blob();
            if (this.#disposed || !accepts() || controller.signal.aborted)
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
