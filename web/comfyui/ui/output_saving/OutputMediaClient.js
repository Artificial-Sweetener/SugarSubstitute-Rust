import { isRecord } from '../types/common.js';
/**
 * Longest side, in image pixels, of the preview chrome uses to mirror an output's colours. A
 * ten-pixel blur hides any detail beyond this, so a larger preview only costs memory.
 */
export const OUTPUT_PREVIEW_LONGEST_SIDE = 320;
/**
 * Build the same-origin URL of an output's small, cacheable still preview.
 *
 * The revision names one immutable output, so the browser may reuse the response for it.
 */
export function outputPreviewUrl(origin, file, revision, longestSide = OUTPUT_PREVIEW_LONGEST_SIDE) {
    const url = new URL('/sugarcubes/output-saving/preview', origin);
    if (url.protocol !== 'http:' && url.protocol !== 'https:')
        throw new Error('The image host is unavailable.');
    url.search = new URLSearchParams({
        filename: file.filename,
        subfolder: file.subfolder,
        type: file.type,
        revision,
        size: String(longestSide),
    }).toString();
    return url.href;
}
/** Keep bounded HTTP media operations reusable across canvas renderers. */
export class OutputMediaClient {
    origin;
    fetch;
    /** Bind one same-origin transport and the actual browser connection address. */
    constructor(origin, fetch) {
        this.origin = origin;
        this.fetch = fetch;
    }
    /** Trust reveal only when both the browser origin and server's guarded capability are local. */
    async profile() {
        return this.#request('/sugarcubes/output-saving/media-profile', async (response) => {
            const value = await response.json();
            if (!response.ok ||
                !isRecord(value) ||
                value.success !== true ||
                !isRecord(value.data) ||
                (value.data.preferred_copy_format !== 'png' &&
                    value.data.preferred_copy_format !== 'jpeg') ||
                typeof value.data.reveal !== 'boolean' ||
                typeof value.data.photoshop !== 'boolean')
                throw new Error('The output image action capabilities are unavailable.');
            const hostname = new URL(this.origin).hostname;
            const local = hostname === 'localhost' || hostname === '[::1]' || /^127(?:\.\d{1,3}){3}$/.test(hostname);
            return Object.freeze({
                preferredCopyFormat: value.data.preferred_copy_format,
                fileAction: local && value.data.reveal ? 'reveal' : 'download',
                photoshop: local && value.data.photoshop,
            });
        });
    }
    /** Fetch a full-resolution PNG or JPEG rendition without creating a permanent companion. */
    async image(file, format, signal) {
        return this.#request('/sugarcubes/output-saving/image', async (response) => {
            if (!response.ok)
                throw await responseError(response, 'The image could not be prepared for copying.');
            const blob = await response.blob();
            const encoded = response.headers.get('X-SugarCubes-Filename');
            const filename = encoded === null ? '' : decodeURIComponent(encoded);
            if (!blob.size ||
                (blob.type !== 'image/png' && blob.type !== 'image/jpeg') ||
                !filename ||
                /[/\\:\0]/.test(filename) ||
                !(blob.type === 'image/png' ? /\.png$/i.test(filename) : /\.jpe?g$/i.test(filename)) ||
                (format === 'png' && blob.type !== 'image/png') ||
                (format === 'jpeg' && blob.type !== 'image/jpeg'))
                throw new Error('The output image rendition response is invalid.');
            return { blob, filename, mimeType: blob.type };
        }, { file, format }, signal);
    }
    /** Request the local adapter to reveal a validated file rather than accepting arbitrary paths. */
    async reveal(file, signal) {
        return this.#request('/sugarcubes/output-saving/reveal', async (response) => {
            const value = await response.json();
            if (!response.ok || !isRecord(value) || value.success !== true)
                throw new Error(isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
                    ? value.error.message
                    : 'The image could not be revealed in its folder.');
        }, file, signal);
    }
    /**
     * Ask the local adapter to open validated output images in Photoshop: one document named
     * `document` holding each image as a layer named by its entry, in order.
     */
    async photoshop(document, layers, signal) {
        return this.#request('/sugarcubes/output-saving/photoshop', async (response) => {
            const value = await response.json();
            if (!response.ok || !isRecord(value) || value.success !== true)
                throw new Error(isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
                    ? value.error.message
                    : 'The images could not be opened in Photoshop.');
        }, { document, layers }, signal);
    }
    /** Build a same-origin download URL whose response supplies the selected rendition's filename. */
    downloadUrl(file) {
        const url = new URL('/sugarcubes/output-saving/image', this.origin);
        if (url.protocol !== 'http:' && url.protocol !== 'https:')
            throw new Error('The image host is unavailable.');
        url.search = new URLSearchParams({ ...file, format: 'preferred' }).toString();
        return url.href;
    }
    /** Bound every request and preserve caller cancellation without leaking timeout or abort listeners. */
    async #request(path, read, value, signal) {
        const controller = new AbortController();
        const abort = () => controller.abort();
        signal?.addEventListener('abort', abort, { once: true });
        if (signal?.aborted)
            abort();
        const timeout = setTimeout(abort, 20000);
        try {
            const response = await this.fetch(path, {
                signal: controller.signal,
                ...(value
                    ? {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify(value),
                    }
                    : {}),
            });
            return await read(response);
        }
        finally {
            clearTimeout(timeout);
            signal?.removeEventListener('abort', abort);
        }
    }
}
/** Preserve structured server errors without relying on arbitrary response markup. */
async function responseError(response, fallback) {
    const value = await response.json();
    return new Error(isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
        ? value.error.message
        : fallback);
}
