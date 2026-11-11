import { isRecord } from '../types/common.js';
/** Share job validation, polling, and cancellation across metadata and acquisition actions. */
export class ModelJobClient {
    #fetch;
    /** Bind the same host transport as the catalog without depending on node presentation. */
    constructor(fetchApi) {
        this.#fetch = fetchApi;
    }
    /** Return completed result evidence and cancel an active server job when its view closes. */
    async observe(response, messages, progress, signal, transfer) {
        let job = decodeJob(await response.json());
        const cancel = () => {
            void this.#fetch(`/sugarcubes/v1/models/jobs/${encodeURIComponent(job.id)}`, {
                method: 'DELETE',
            }).catch((error) => console.error('SugarCubes model job cancellation failed.', error));
        };
        if (signal.aborted) {
            cancel();
            throw new DOMException('Cancelled', 'AbortError');
        }
        signal.addEventListener('abort', cancel, { once: true });
        try {
            while (job.status === 'queued' || job.status === 'running') {
                transfer?.({
                    message: job.status === 'queued' ? messages.queued : messages.running,
                    percentage: job.percentage,
                });
                progress(job.status === 'queued'
                    ? messages.queued
                    : messages.running + (job.percentage === null ? '' : ` ${job.percentage}%`));
                await abortableModelDelay(400, signal);
                const next = await this.#fetch(`/sugarcubes/v1/models/jobs/${encodeURIComponent(job.id)}`, {
                    signal,
                });
                if (!next.ok)
                    throw new Error('Model job status is unavailable.');
                job = decodeJob(await next.json());
            }
            if (job.status !== 'completed')
                throw new Error(job.error || 'The model operation did not complete.');
            return job.result;
        }
        finally {
            signal.removeEventListener('abort', cancel);
        }
    }
}
/** Reject unknown job identities and states before addressing server resources. */
function decodeJob(value) {
    if (!isRecord(value) || typeof value.jobId !== 'string' || !/^[a-f0-9]{32}$/.test(value.jobId))
        throw new Error('Invalid model job response.');
    const status = value.status;
    if (status !== 'queued' &&
        status !== 'running' &&
        status !== 'completed' &&
        status !== 'failed' &&
        status !== 'cancelled')
        throw new Error('Invalid model job state.');
    return {
        id: value.jobId,
        status,
        error: typeof value.error === 'string' ? value.error : '',
        result: isRecord(value.result) ? value.result : {},
        percentage: decodeProgress(value.progress),
    };
}
/** Validate streamed byte evidence without trusting caller-supplied progress strings. */
function decodeProgress(value) {
    if (value === undefined || (isRecord(value) && Object.keys(value).length === 0))
        return null;
    if (!isRecord(value))
        throw new Error('Invalid model transfer progress.');
    const downloaded = value.bytesDownloaded;
    const total = value.bytesTotal;
    if (typeof downloaded !== 'number' ||
        typeof total !== 'number' ||
        !Number.isSafeInteger(downloaded) ||
        !Number.isSafeInteger(total) ||
        downloaded < 0 ||
        total <= 0 ||
        downloaded > 128 * 1024 ** 3 + 1024 ||
        total > 128 * 1024 ** 3)
        throw new Error('Invalid model transfer progress.');
    return Math.min(100, Math.floor((downloaded / total) * 100));
}
/** Release retry and polling timers immediately when the owning presentation closes. */
export function abortableModelDelay(milliseconds, signal) {
    return new Promise((resolve, reject) => {
        if (signal.aborted) {
            reject(new DOMException('Cancelled', 'AbortError'));
            return;
        }
        const abort = () => {
            clearTimeout(timer);
            reject(new DOMException('Cancelled', 'AbortError'));
        };
        const timer = setTimeout(() => {
            signal.removeEventListener('abort', abort);
            resolve();
        }, milliseconds);
        signal.addEventListener('abort', abort, { once: true });
    });
}
