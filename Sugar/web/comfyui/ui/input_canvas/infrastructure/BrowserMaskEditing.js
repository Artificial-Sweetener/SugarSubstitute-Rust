import { decodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
import { InputRasterTransformCache } from '../application/InputRasterTransformCache.js';
import { readMaskEditSettings } from '../domain/MaskEditSettings.js';
import { readMaskEditingResult } from './MaskEditingTransport.js';
import { isRecord } from '../../types/common.js';
/** Own one worker and one byte-bounded cache for both preview and submission callers. */
export class BrowserMaskEditing {
    createWorker;
    #cache = new InputRasterTransformCache();
    #pending = new Map();
    #queue = [];
    #nextJob = 0;
    #active = null;
    #worker = null;
    #disposed = false;
    /** Create a dedicated worker lazily so canvases without mask editing allocate none. */
    constructor(createWorker = () => new Worker(new URL('./MaskEditingWorker.js', import.meta.url), { type: 'module' })) {
        this.createWorker = createWorker;
    }
    /** Share identical work while each preview owner coalesces its own obsolete intentions. */
    prepare(source, settings, purpose) {
        if (this.#disposed)
            return Promise.reject(new Error('Mask preparation is closed.'));
        const accepted = readMaskEditSettings(settings);
        if (source.format !== 'coverage8')
            return Promise.reject(new Error('Mask preparation requires coverage.'));
        return this.#cache.read([source], JSON.stringify(accepted), () => this.#schedule(source, accepted, purpose), (value) => value.mask.data.length * 2);
    }
    /** Admit one deduplicated operation to the worker scheduling boundary. */
    #schedule(source, settings, purpose) {
        if (this.#pending.size >= 16)
            return Promise.reject(new Error('Too many pending mask preparations.'));
        let resolve = () => { }, reject = () => { };
        const promise = new Promise((success, failure) => {
            resolve = success;
            reject = failure;
        });
        const job = {
            id: ++this.#nextJob,
            source,
            settings,
            purpose,
            promise,
            resolve,
            reject,
        };
        this.#pending.set(job.id, job);
        if (purpose === 'submission')
            this.#queue.unshift(job);
        else
            this.#queue.push(job);
        this.#drain();
        return promise;
    }
    /** Reject outstanding work before releasing the browser-owned worker. */
    dispose() {
        this.#disposed = true;
        this.#fail(new Error('Mask preparation is closed.'));
        this.#cache.clear();
    }
    /** Transfer only one full-resolution source at a time, keeping pending slider intents lightweight. */
    #drain() {
        if (this.#disposed || this.#active)
            return;
        const job = this.#queue.shift();
        if (!job)
            return;
        this.#active = job;
        try {
            if (!this.#worker) {
                this.#worker = this.createWorker();
                this.#worker.addEventListener('message', (event) => this.#receive(event.data));
                this.#worker.addEventListener('error', (event) => this.#fail(new Error(event.message || 'Mask worker failed.')));
            }
            decodeCanvasRaster(job.source);
            this.#worker.postMessage({ id: job.id, source: job.source, settings: job.settings });
        }
        catch (error) {
            this.#fail(error);
        }
    }
    /** Admit the exact current result and evict retained pixels before exceeding the cache budget. */
    #receive(value) {
        const job = this.#active;
        if (!job)
            return;
        try {
            if (!isRecord(value) || value.id !== job.id)
                throw new Error('Invalid mask worker response identity.');
            if (typeof value.error === 'string')
                throw new Error(value.error);
            const result = readMaskEditingResult(value.result, job.source);
            job.resolve(result);
        }
        catch (error) {
            job.reject(error);
        }
        this.#pending.delete(job.id);
        this.#active = null;
        this.#drain();
    }
    /** Fail explicit callers together when the external worker cannot continue. */
    #fail(error) {
        this.#worker?.terminate();
        this.#worker = null;
        for (const job of this.#pending.values())
            job.reject(error);
        this.#pending.clear();
        this.#queue.length = 0;
        this.#active = null;
    }
}
