import { DesktopSessionRefusal } from '../domain/DesktopSession.js';
/** Prefer the verified backend original so its existing reveal, rendition and Photoshop capabilities remain available. */
export class ComfyDesktopArtifacts {
    backend;
    retained;
    constructor(backend, retained) {
        this.backend = backend;
        this.retained = retained;
    }
    async receipt(file, cancellation) {
        return this.#request(cancellation, (signal) => this.backend.receipt(file, signal));
    }
    async resolve(output, binding, cancellation) {
        const backend = await this.#request(cancellation, (signal) => this.backend.resolve(output, signal));
        if (backend.available || output.assetDigest === null || cancellation.cancelled())
            return backend;
        try {
            const presentation = await this.retained.presentation(output, binding, cancellation);
            return {
                available: true,
                receipt: {
                    file: output.file,
                    digest: output.digest,
                    bytes: output.bytes,
                    width: output.width,
                    height: output.height,
                    media: output.media,
                    mime: output.mime,
                    metadataEmbedded: output.metadataEmbedded,
                    animated: presentation.animated,
                },
                url: presentation.url,
            };
        }
        catch (error) {
            if (error instanceof DesktopSessionRefusal) {
                if (error.code === 'cancelled')
                    throw error;
                return { available: false, code: error.code, message: error.message };
            }
            throw error;
        }
    }
    /** Tie every backend request to the owning protocol cancellation rather than a detached request lifetime. */
    async #request(cancellation, request) {
        const controller = new AbortController();
        const stop = cancellation.subscribe(() => controller.abort());
        try {
            return await request(controller.signal);
        }
        finally {
            stop();
        }
    }
}
