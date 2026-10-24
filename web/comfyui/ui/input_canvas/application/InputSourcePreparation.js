import { canvasInputSourceKey } from '../../core/embedded/CanvasRuntime.js';
import { maskSourceImage } from '../domain/MaskSourcePixels.js';
/** Adapt source capture to a file-loading field without changing source channels or the loader's settings. */
export class InputSourcePreparation {
    capture;
    assets;
    /** Reuse the generic capture owner also used by committed preview updates. */
    constructor(capture, assets) {
        this.capture = capture;
        this.assets = assets;
    }
    /** Capture all targets before upload and preserve each caller's exact entry identity. */
    async prepare(entries) {
        const snapshots = await this.capture.capture(entries, 0), staged = new Map(), files = new Map();
        for (const entry of entries) {
            const snapshot = snapshots.get(entry.key);
            if (!snapshot)
                throw new Error('Missing captured input source.');
            const key = JSON.stringify([canvasInputSourceKey(entry.source), snapshot.pixels.format]);
            let file = staged.get(key);
            if (!file) {
                file = this.assets.stage(maskSourceImage(snapshot.pixels));
                staged.set(key, file);
            }
            files.set(entry.key, await file);
        }
        return files;
    }
}
