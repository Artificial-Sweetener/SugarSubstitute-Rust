const idle = Object.freeze({ visible: false, workflow: 0, sampler: 0 });
/** Retain one backend execution while the existing run registry owns workflow attribution. */
export class WorkflowProgressStore {
    run;
    #completed = new Set();
    #listeners = new Set();
    #active = null;
    #sampler = 0;
    #node = '';
    /** Resolve immutable submitted node paths from the authoritative accepted-run owner. */
    constructor(run) {
        this.run = run;
    }
    /** Show only the running workflow generation, including acceptance arriving after its first event. */
    read(context) {
        const run = this.#active ? this.run(this.#active) : null;
        if (!context ||
            !run ||
            run.session.sessionId !== context.sessionId ||
            run.session.generation !== context.generation ||
            ['complete', 'cancelled', 'error'].includes(run.status))
            return idle;
        const paths = new Set(run.sources.map((source) => source.executionPath));
        const completed = [...this.#completed].filter((path) => paths.has(path)).length;
        return Object.freeze({
            visible: true,
            workflow: paths.size ? (completed / paths.size) * 100 : 0,
            sampler: this.#sampler,
        });
    }
    /** Replace the single executing job without retaining stale progress from its predecessor. */
    started(id) {
        this.#active = id;
        this.#completed.clear();
        this.#sampler = 0;
        this.#node = '';
        this.refresh();
    }
    /** Count exact completed and cached execution paths once. */
    completed(id, paths) {
        if (id !== this.#active)
            return;
        for (const path of paths)
            this.#completed.add(path);
        this.refresh();
    }
    /** Apply validated node progress, resetting naturally when a new sampler starts. */
    sampler(id, node, value, maximum) {
        if (id !== this.#active || !Number.isFinite(value) || !Number.isFinite(maximum) || maximum <= 0)
            return;
        this.#node = node;
        this.#sampler = Math.max(0, Math.min(100, (value / maximum) * 100));
        this.refresh();
    }
    /** Clear a finished sampler while retaining whole-workflow progress. */
    nodeFinished(id, paths) {
        if (id !== this.#active)
            return;
        if (paths.includes(this.#node))
            this.#sampler = 0;
        this.completed(id, paths);
    }
    /** Ignore a delayed terminal notification belonging to another job. */
    terminal(id) {
        if (id === this.#active)
            this.reset();
    }
    /** Retire the displayed execution on completion or connection loss. */
    reset() {
        this.#active = null;
        this.#completed.clear();
        this.#sampler = 0;
        this.#node = '';
        this.refresh();
    }
    /** Publish acceptance and progress updates without graph discovery or tab relayout. */
    refresh() {
        for (const listener of this.#listeners)
            listener();
    }
    /** Release exactly one presentation subscription. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Release retained paths and observers with the execution runtime. */
    dispose() {
        this.#listeners.clear();
        this.reset();
    }
}
