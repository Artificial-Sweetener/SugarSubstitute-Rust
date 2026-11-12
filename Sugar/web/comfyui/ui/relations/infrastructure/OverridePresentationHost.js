import { registerPluginModePreference } from '../../surface/frame/PluginModePreference.js';
/** Keep mode precedence at one owner instead of competing frame callbacks. */
export class OverridePresentationHost {
    toolbar;
    #inline = false;
    #plugin = false;
    #release = null;
    /** Bind the existing controls without creating another relation or value owner. */
    constructor(toolbar) {
        this.toolbar = toolbar;
    }
    /** Restore Plugin Mode once Comfy settings are ready. */
    setup(app, document, report, embedded) {
        if (this.#release)
            return;
        this.#release = registerPluginModePreference(app, document, (enabled) => {
            this.#plugin = enabled;
            this.#present();
        }, report, embedded);
    }
    /** Retain frame intent while Plugin Mode owns the override card. */
    setInline(inline) {
        this.#inline = inline;
        this.#present();
    }
    /** Release document presentation without changing the saved host preference. */
    dispose() {
        this.#release?.();
        this.#release = null;
    }
    /** Apply mode precedence through the existing control presentation boundary. */
    #present() {
        this.toolbar.setPresentation(this.#plugin ? 'plugin' : this.#inline ? 'substitute' : 'comfy');
        this.toolbar.reconcile();
    }
}
