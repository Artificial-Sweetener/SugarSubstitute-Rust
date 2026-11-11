import { isRecord } from '../../types/common.js';
/** Extension setup registers observers and returns before Comfy's own workflow/tab restoration. */
export class ComfyDesktopReadiness {
    sessions;
    #manager;
    #settings;
    #listeners = new Set();
    #stops = [];
    #snapshot = Object.freeze({ committed: false, workflowPersistence: false });
    /** Require the exposed workspace subscription and settings change surface at the dynamic boundary. */
    constructor(app, sessions) {
        this.sessions = sessions;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        const ui = isRecord(app) && isRecord(app.ui) ? app.ui : null;
        const settings = ui && isRecord(ui.settings) ? ui.settings : null;
        if (!manager ||
            typeof manager.spinner !== 'boolean' ||
            typeof manager.$subscribe !== 'function' ||
            !settings ||
            typeof settings.getSettingValue !== 'function' ||
            typeof settings.addEventListener !== 'function' ||
            typeof settings.removeEventListener !== 'function')
            throw new TypeError('Committed Comfy desktop startup observation is unavailable.');
        this.#manager = manager;
        this.#settings = settings;
    }
    /** Subscribe before inspecting state so delayed startup completion cannot be lost. */
    install() {
        if (this.#stops.length)
            return;
        const subscribe = this.#manager.$subscribe;
        if (typeof subscribe !== 'function')
            throw new TypeError('Comfy workspace subscription retired.');
        const stop = subscribe.call(this.#manager, this.#refresh, {
            detached: true,
            flush: 'sync',
        });
        if (typeof stop !== 'function')
            throw new TypeError('Comfy workspace unsubscribe is unavailable.');
        this.#stops.push(() => stop(), this.sessions.subscribe(this.#refresh));
        const add = this.#settings.addEventListener;
        const remove = this.#settings.removeEventListener;
        if (typeof add !== 'function' || typeof remove !== 'function')
            throw new TypeError('Comfy workflow persistence observation retired.');
        add.call(this.#settings, 'Comfy.Workflow.Persist.change', this.#refresh);
        this.#stops.push(() => remove.call(this.#settings, 'Comfy.Workflow.Persist.change', this.#refresh));
        this.#refresh();
    }
    /** Read the latest explicit commitment facts independently of presentation visibility. */
    read() {
        return this.#snapshot;
    }
    /** Notify only semantic readiness/opt-out transitions, not unrelated workspace mutations. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Retire every host observation when its owning extension page leaves. */
    dispose() {
        for (const stop of this.#stops.splice(0))
            stop();
        this.#listeners.clear();
    }
    /** Combine built-in startup completion with the existing asynchronous graph commitment fence. */
    #refresh = () => {
        const get = this.#settings.getSettingValue;
        if (typeof get !== 'function')
            throw new TypeError('Comfy workflow persistence setting retired.');
        const value = get.call(this.#settings, 'Comfy.Workflow.Persist', true);
        if (typeof value !== 'boolean')
            throw new TypeError('Comfy workflow persistence setting is invalid.');
        const snapshot = this.sessions.read();
        const next = {
            committed: this.#manager.spinner === false && !snapshot.loading && snapshot.active !== null,
            workflowPersistence: value,
        };
        if (next.committed === this.#snapshot.committed &&
            next.workflowPersistence === this.#snapshot.workflowPersistence)
            return;
        this.#snapshot = Object.freeze(next);
        for (const listener of this.#listeners)
            listener();
    };
}
