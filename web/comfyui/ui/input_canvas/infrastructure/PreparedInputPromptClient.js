import { isRecord } from '../../types/common.js';
/** Keep graph projection policy on the same backend owner used by direct Cube execution. */
export class PreparedInputPromptClient {
    fetch;
    /** Inject same-origin transport for bounded, deterministic failure behavior. */
    constructor(fetch) {
        this.fetch = fetch;
    }
    /** Return the canonical derived prompt while preserving authored workflow data and loader settings. */
    async prepare(prompt, assets) {
        const response = await this.fetch('/sugarcubes/v2/canvas-inputs/prepare', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ prompt, assets }),
            signal: AbortSignal.timeout(30000),
        });
        const payload = await response.json();
        if (!response.ok || !isRecord(payload) || !isRecord(payload.prompt))
            throw new Error(isRecord(payload) && typeof payload.error === 'string'
                ? payload.error
                : 'Input preparation is unavailable.');
        return payload.prompt;
    }
}
