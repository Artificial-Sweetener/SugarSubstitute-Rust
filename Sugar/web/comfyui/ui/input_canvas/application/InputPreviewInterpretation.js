import { usesNativeMaskChannels } from '../domain/InputLoader.js';
import { nativeMaskSource } from '../domain/MaskSourcePixels.js';
import { imagePreviewRaster } from '../domain/ImagePreviewRaster.js';
/** Apply mask semantics while image consumers retain actual source content and transparency. */
export class InputPreviewInterpretation {
    loader;
    files;
    readSettings;
    #interpreted = new WeakMap();
    /** Read current authored settings from their owning node only when interpretation is requested. */
    constructor(loader, files, readSettings) {
        this.loader = loader;
        this.files = files;
        this.readSettings = readSettings;
    }
    /** Expose a current immutable settings snapshot for view invalidation. */
    get settings() {
        return this.readSettings();
    }
    /** Preserve custom-loader interpretation at native resolution while bounding ordinary display capture. */
    captureSize(maximumSize) {
        return this.loader.kind === 'image' || usesNativeMaskChannels(this.loader) ? maximumSize : 0;
    }
    /** Decode an actual file without competing with its loader's file codec. */
    file(file, maximumSize) {
        return this.files.load(file, this.settings, maximumSize);
    }
    /** Reinterpret raw mask channels locally while image capture already supplies selected image pixels. */
    canvas(source, maximumSize) {
        const settings = this.settings, key = JSON.stringify([settings, maximumSize]);
        const cached = this.#interpreted.get(source);
        if (cached?.key === key)
            return cached.pixels;
        const pixels = this.#interpret(source, settings, maximumSize);
        this.#interpreted.set(source, { key, pixels });
        void pixels.catch(() => {
            if (this.#interpreted.get(source)?.pixels === pixels)
                this.#interpreted.delete(source);
        });
        return pixels;
    }
    /** Apply the loader's actual settings after sharing identical in-flight interpretation. */
    async #interpret(source, settings, maximumSize) {
        if (this.loader.kind === 'image') {
            if (source.format !== 'rgba8')
                throw new Error('The image source did not contain image pixels.');
            return imagePreviewRaster(source, maximumSize);
        }
        const native = nativeMaskSource(source, this.loader, settings, false);
        if (native)
            return native;
        return await this.files.interpret(source, settings, maximumSize);
    }
}
