import { isRecord } from '../../types/common.js';
import { encodeSourcePng } from '../../core/embedded/CanvasPngEncoding.js';
import { InputRasterTransformCache } from '../application/InputRasterTransformCache.js';
/** Own one upload policy shared by source controls, migration and queue-time canvas capture. */
export class BrowserCanvasSourceAssets {
    fetch;
    #verified = new Set();
    #staged = new InputRasterTransformCache();
    /** Inject same-origin transport without giving application code browser networking APIs. */
    constructor(fetch) {
        this.fetch = fetch;
    }
    /** Reuse an immutable upload when source pixels survive capture or generation boundaries unchanged. */
    stage(pixels) {
        return this.#staged.read([pixels], '', async () => await this.#upload(await encodeSourcePng(pixels), `source-${crypto.randomUUID()}.png`), (file) => file.length * 2);
    }
    /** Upload the user's selected file without decoding or changing its original channel data. */
    async file(file) {
        return await this.#upload(file, file.name);
    }
    /** Recognize real uploads without inventing choices for the native file widget. */
    verified(file) {
        return this.#verified.has(file);
    }
    /** Confirm restored source files through Comfy's media boundary before publishing native choices. */
    async verify(file) {
        if (this.#verified.has(file))
            return true;
        const match = /^(.*?)(?: \[(input|output|temp)\])?$/.exec(file);
        const name = (match?.[1] ?? file).replaceAll('\\', '/'), parts = name.split('/'), filename = parts.pop() ?? '';
        const response = await this.fetch('/view?' +
            new URLSearchParams({
                filename,
                subfolder: parts.join('/'),
                type: match?.[2] ?? 'input',
            }).toString(), { method: 'HEAD', signal: AbortSignal.timeout(30000) });
        if (response.ok)
            this.#verified.add(file);
        return response.ok;
    }
    /** Validate server-chosen relative input identity before allowing it into persisted source state. */
    async #upload(blob, name) {
        const form = new FormData();
        form.append('image', blob, name);
        form.append('type', 'input');
        form.append('subfolder', 'sugarcubes/sources');
        form.append('overwrite', 'false');
        const response = await this.fetch('/upload/image', {
            method: 'POST',
            body: form,
            signal: AbortSignal.timeout(30000),
        });
        const result = await response.json();
        if (!response.ok ||
            !isRecord(result) ||
            typeof result.name !== 'string' ||
            !result.name ||
            /[\\/:]|\p{Cc}/u.test(result.name) ||
            result.name === '.' ||
            result.name === '..' ||
            result.subfolder !== 'sugarcubes/sources' ||
            result.type !== 'input')
            throw new Error('Comfy could not upload the source image.');
        const file = `${result.subfolder}/${result.name}`;
        this.#verified.add(file);
        return file;
    }
}
