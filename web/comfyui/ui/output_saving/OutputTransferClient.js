import { isRecord } from '../types/common.js';
/** Keep network access and response validation independent of a canvas or operating system. */
export class OutputTransferClient {
    fetch;
    /** Bind an explicit same-origin transport rather than reading renderer-owned preferences. */
    constructor(fetch) {
        this.fetch = fetch;
    }
    /** Revalidate a file and apply current preferences with bounded, caller-cancellable network work. */
    async choose(file, signal) {
        const controller = new AbortController();
        const abort = () => controller.abort();
        signal?.addEventListener('abort', abort, { once: true });
        if (signal?.aborted)
            abort();
        const timer = setTimeout(abort, 20000);
        try {
            const response = await this.fetch('/sugarcubes/output-saving/transfer', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(file),
                signal: controller.signal,
            });
            const value = await response.json();
            if (!response.ok || !isRecord(value) || value.success !== true) {
                const message = isRecord(value) && isRecord(value.error) ? value.error.message : null;
                throw new Error(typeof message === 'string' ? message : 'This output image is unavailable for transfer.');
            }
            return decodeSelection(value.data);
        }
        finally {
            clearTimeout(timer);
            signal?.removeEventListener('abort', abort);
        }
    }
}
/** Preserve full-fidelity file URLs without native lossy preview parameters. */
export function outputFileUrl(origin, file) {
    const url = new URL('/view', origin);
    if (url.protocol !== 'http:' && url.protocol !== 'https:')
        throw new Error('The image host is unavailable.');
    url.search = new URLSearchParams({ ...file }).toString();
    return url.href;
}
/** Admit exact portable image identities without accepting external URLs or filesystem paths. */
function decodeSelection(value) {
    if (!isRecord(value) || !isRecord(value.file))
        throw new Error('Invalid output transfer response.');
    const { filename, subfolder, type } = value.file;
    if (typeof filename !== 'string' ||
        !filename ||
        /[/\\:]/.test(filename) ||
        filename.includes('\0') ||
        typeof subfolder !== 'string' ||
        subfolder.includes(':') ||
        subfolder.includes('\0') ||
        subfolder.startsWith('/') ||
        subfolder.split(/[/\\]/).some((part) => part === '.' || part === '..') ||
        (type !== 'input' && type !== 'output' && type !== 'temp') ||
        (value.mime_type !== 'image/png' &&
            value.mime_type !== 'image/jpeg' &&
            value.mime_type !== 'image/webp'))
        throw new Error('Invalid output transfer response.');
    return Object.freeze({
        file: Object.freeze({ filename, subfolder, type }),
        mimeType: value.mime_type,
    });
}
