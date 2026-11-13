/** Own one cancellable frame for the renderer's pending label measurements. */
export class NativeLinkLabelLayout {
    document;
    reportError;
    #pending = new Set();
    #frame = null;
    /** Receive the rendering document and its actionable error boundary. */
    constructor(document, reportError) {
        this.document = document;
        this.reportError = reportError;
    }
    /** Coalesce replacement and resize notifications without retaining inactive controls. */
    invalidate(measure) {
        const windowRef = this.document.defaultView;
        if (!windowRef)
            return;
        this.#pending.add(measure);
        if (this.#frame !== null)
            return;
        this.#frame = windowRef.requestAnimationFrame(this.#flush);
    }
    /** Release pending work and its frame when the last control leaves. */
    release(measure) {
        this.#pending.delete(measure);
        if (this.#pending.size || this.#frame === null)
            return;
        this.document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
    }
    /** Complete every read before writes so siblings share one browser layout. */
    #flush = () => {
        this.#frame = null;
        const measures = [...this.#pending];
        this.#pending.clear();
        const commits = [];
        for (const measure of measures) {
            try {
                const commit = measure();
                if (commit)
                    commits.push(commit);
            }
            catch (error) {
                this.reportError(error);
            }
        }
        for (const commit of commits) {
            try {
                commit();
            }
            catch (error) {
                this.reportError(error);
            }
        }
    };
}
