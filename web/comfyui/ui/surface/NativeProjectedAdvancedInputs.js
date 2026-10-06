import { nativeAdvancedInputControl } from './NativeAdvancedInputControl.js';
/** Preserve native footer rendering while sharing advanced state with context actions. */
export class NativeProjectedAdvancedInputs {
    #target;
    #node;
    #control;
    /** Bind one node rather than routing projected IDs through the active graph. */
    constructor(target, node, refresh) {
        this.#target = target;
        this.#node = node;
        this.#control = nativeAdvancedInputControl(node, refresh);
        target.addEventListener('click', this.#toggle, true);
    }
    /** Keep advanced controls available only on an expanded card with owned advanced fields. */
    reconcile(footer) {
        const shown = this.#control !== null &&
            this.#node.flags?.collapsed !== true &&
            footer.querySelector('[data-testid="advanced-inputs-button"]') !== null;
        footer.hidden = !shown;
        if (shown)
            footer.style.removeProperty('display');
        else
            footer.style.setProperty('display', 'none', 'important');
    }
    /** Return footer interaction to Comfy when its projected card is unmounted. */
    dispose() {
        this.#target.removeEventListener('click', this.#toggle, true);
    }
    /** Apply the same exact-node transition used by the context menu. */
    #toggle = (event) => {
        if (!(event.target instanceof Element) || !this.#control)
            return;
        const button = event.target.closest('[data-testid="advanced-inputs-button"]');
        if (!button || !this.#target.contains(button) || button.disabled)
            return;
        event.stopImmediatePropagation();
        this.#control.setShown(!this.#control.shown());
    };
}
