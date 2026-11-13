import { ModelPageActivity } from './ModelPageActivity.js';
/** Keep browser restoration and focus independent of model-specific provider refreshes. */
export class ModelUpdateLifecycle {
    #window;
    #updates;
    #report;
    #page;
    #ready = false;
    #generation = 0;
    #loading = Promise.resolve();
    /** Bind the browser lifecycle and validated update port. */
    constructor(window, updates, report) {
        this.#window = window;
        this.#updates = updates;
        this.#report = report;
        this.#page = new ModelPageActivity(window, this.#activity, () => this.dispose());
    }
    /** Read durable consent on activation and observe focus through reversible event listeners. */
    start() {
        this.#page.start();
    }
    /** Remove browser subscriptions without cancelling shared server-owned provider checks. */
    dispose() {
        this.#page.dispose();
    }
    /** Reload consent in order after restoration so stale reads cannot overwrite a newer decision. */
    #activity = (active) => {
        const generation = ++this.#generation;
        this.#ready = false;
        this.#window.removeEventListener('focus', this.#focus);
        if (!active)
            return;
        this.#window.addEventListener('focus', this.#focus);
        this.#loading = this.#loading
            .then(async () => {
            if (!this.#page.active || generation !== this.#generation)
                return;
            await this.#updates.load();
            if (!this.#page.active || generation !== this.#generation)
                return;
            this.#ready = true;
        })
            .catch(this.#report);
    };
    /** Reload prepared server state on focus without a CivitAI request. */
    #focus = () => {
        if (this.#page.active && this.#ready)
            this.#loading = this.#loading
                .then(() => (this.#page.active ? this.#updates.load() : undefined))
                .catch(this.#report);
    };
}
