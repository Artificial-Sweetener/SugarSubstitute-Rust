import { OutputMediaClient } from './OutputMediaClient.js';
import { OutputMediaProfileSource } from './OutputMediaProfileSource.js';
import { BrowserRetainedOutputTransfer, retainedOutputCapabilities, } from './BrowserRetainedOutputTransfer.js';
import { OutputTransferClient, outputFileUrl, } from './OutputTransferClient.js';
/** Coordinate the shared file selection with browser-owned transport and clipboard boundaries. */
export class BrowserOutputTransfer {
    origin;
    fetch;
    clipboard;
    download;
    rendition;
    #client;
    #media;
    #profile;
    #retained;
    #disposed = false;
    /** Bind platform collaborators explicitly so native canvases can use the same selection endpoint. */
    constructor(origin, fetch, clipboard, download, rendition) {
        this.origin = origin;
        this.fetch = fetch;
        this.clipboard = clipboard;
        this.download = download;
        this.rendition = rendition;
        this.#client = new OutputTransferClient(fetch);
        this.#media = new OutputMediaClient(origin, fetch);
        this.#profile = new OutputMediaProfileSource(() => this.#media.profile());
        this.#retained = new BrowserRetainedOutputTransfer(origin, fetch);
    }
    /** Expose a conservative cached profile without network work when a context menu opens. */
    profile(retained) {
        const profile = this.#profile.read();
        return retained ? { ...profile, fileAction: 'download', photoshop: false } : profile;
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
    async copy(file, accepts, format, signal, retained) {
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        if (retained)
            return this.#copyRetained(file, retained, accepts, format, signal);
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
    async fileAction(file, accepts, signal, content = 'image', retained) {
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        if (retained) {
            if (!retainedOutputCapabilities(retained).download)
                return false;
            const url = await this.#retained.download(file, retained, signal);
            if (this.#disposed || signal?.aborted || !accepts())
                return false;
            this.download.start(url, file.filename);
            return true;
        }
        if (this.#profile.read().fileAction === 'reveal')
            await this.#media.reveal(file, signal);
        else
            this.download.start(content === 'image' ? this.#media.downloadUrl(file) : outputFileUrl(this.origin, file));
        return true;
    }
    /** Open still images in Photoshop through the local backend while their outputs stay current. */
    async photoshop(document, layers, accepts, signal) {
        if (this.#disposed || signal?.aborted || !accepts() || !this.#profile.read().photoshop)
            return false;
        if (layers.some((layer) => layer.retained))
            return false;
        await this.#media.photoshop(document, layers, signal);
        return true;
    }
    /** Retire profile listeners and refuse late clipboard writes after the browser host is torn down. */
    dispose() {
        this.#disposed = true;
        this.#profile.dispose();
        this.#retained.dispose();
    }
    /** Prepare exact preferred file bytes for a synchronous browser drag without retaining them here. */
    async drag(file, signal, retained) {
        if (retained) {
            if (this.#disposed || signal.aborted || !retainedOutputCapabilities(retained).drag)
                return null;
            const image = await this.#retained.image(file, retained, signal);
            if (this.#disposed || signal.aborted)
                return null;
            return {
                file: new File([image.blob], image.filename, { type: image.mimeType }),
                url: image.url,
            };
        }
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
    /** Preserve current copy policy while acquiring original pixels from their native retained owner. */
    async #copyRetained(file, retained, accepts, format, signal) {
        if (!retainedOutputCapabilities(retained).copy)
            return false;
        const image = await this.#retained.image(file, retained, signal);
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        const policy = await this.#media.copyPolicy(signal);
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        const desired = (format === 'preferred' ? policy.format : format) === 'jpeg' ? 'image/jpeg' : 'image/png';
        let blob = image.blob;
        if (image.mimeType !== desired) {
            if (!this.rendition)
                throw new Error('Retained output image encoding is unavailable.');
            blob = await this.rendition.encode(blob, desired, policy.quality, image.dimensions, signal);
        }
        if (this.#disposed || signal?.aborted || !accepts())
            return false;
        await this.clipboard.write(blob, desired);
        return true;
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
