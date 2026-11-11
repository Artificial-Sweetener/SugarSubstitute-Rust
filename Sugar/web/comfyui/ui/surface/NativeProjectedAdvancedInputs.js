import { nativeAdvancedInputControl } from './NativeAdvancedInputControl.js';
import { prepareNativePresentation } from './NativePresentationReady.js';
const mounts = new WeakMap();
/** Resolve a positively registered projection without consulting graph IDs or the application tree. */
export function readProjectedAdvancedInputMount(root) {
    const target = root.closest('.sugarcubes-native-node-card');
    return target ? (mounts.get(target) ?? null) : null;
}
/** Preserve native footer rendering while sharing advanced state with context actions. */
export class NativeProjectedAdvancedInputs {
    #target;
    #node;
    #control;
    #presentation = null;
    /** Bind one node rather than routing projected IDs through the active graph. */
    constructor(target, node, refresh, motion) {
        this.#target = target;
        this.#node = node;
        this.#control = nativeAdvancedInputControl(node, () => {
            refresh();
            const root = target.querySelector('.lg-node');
            if (root)
                prepareNativePresentation(root);
        });
        if (this.#control)
            mounts.set(target, {
                target,
                control: this.#control,
                motion,
                present: (toggle, dispose) => {
                    this.#presentation?.dispose();
                    const presentation = { toggle, dispose };
                    this.#presentation = presentation;
                    return () => {
                        if (this.#presentation === presentation)
                            this.#presentation = null;
                    };
                },
            });
        target.addEventListener('click', this.#toggle, true);
    }
    /** Keep advanced controls available only on an expanded card with owned advanced fields. */
    reconcile(footer) {
        const shown = this.#control !== null &&
            this.#node.flags?.collapsed !== true &&
            footer.querySelector('[data-testid="advanced-inputs-button"]') !== null;
        if (footer.hidden === shown)
            footer.hidden = !shown;
        if (shown)
            footer.style.removeProperty('display');
        else if (footer.style.display !== 'none' ||
            footer.style.getPropertyPriority('display') !== 'important')
            footer.style.setProperty('display', 'none', 'important');
    }
    /** Return footer interaction to Comfy when its projected card is unmounted. */
    dispose() {
        mounts.delete(this.#target);
        this.#presentation?.dispose();
        this.#presentation = null;
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
        if (this.#presentation)
            this.#presentation.toggle();
        else
            this.#control.setShown(!this.#control.shown());
    };
}
