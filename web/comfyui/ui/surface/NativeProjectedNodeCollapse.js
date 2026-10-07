import { isNativeWidgetPresented } from './NativeWidgetPresentation.js';
import { cubeFaceRevealableWidgets } from './CubeFaceWidgetPolicy.js';
import { settleNativeDisclosure } from './NativeDisclosureSettlement.js';
/** Adapt a native collapse button while leaving flags, history, and graph geometry with Comfy. */
export class NativeProjectedNodeCollapse {
    #target;
    #node;
    #refresh;
    #motion;
    /** Bind one projected card and its native renderer refresh. */
    constructor(target, node, refresh, motion) {
        this.#target = target;
        this.#node = node;
        this.#refresh = refresh;
        this.#motion = motion;
        target.addEventListener('click', this.#collapse, true);
    }
    /** Reflect the native node's collapse capability in the existing control. */
    reconcile(root, hasContent = cubeFaceRevealableWidgets(this.#node).some(isNativeWidgetPresented)) {
        const button = root.querySelector('[data-testid="node-collapse-button"]');
        if (button) {
            if (button.hidden === hasContent)
                button.hidden = !hasContent;
            if (button.hidden) {
                if (button.style.display !== 'none' ||
                    button.style.getPropertyPriority('display') !== 'important')
                    button.style.setProperty('display', 'none', 'important');
            }
            else
                button.style.removeProperty('display');
            const disabled = button.hidden ||
                typeof this.#node.collapse !== 'function' ||
                this.#node.collapsible === false;
            if (button.disabled !== disabled)
                button.disabled = disabled;
        }
    }
    /** Release the host-facing listener before the Vue mount is unmounted. */
    dispose() {
        this.#target.removeEventListener('click', this.#collapse, true);
    }
    /** Invoke the authoritative node transition without consulting ambiguous numeric IDs. */
    #collapse = (event) => {
        if (!(event.target instanceof Element))
            return;
        const button = event.target.closest('[data-testid="node-collapse-button"]');
        if (!button ||
            !this.#target.contains(button) ||
            button.disabled ||
            typeof this.#node.collapse !== 'function')
            return;
        event.stopImmediatePropagation();
        this.#motion.toggle(this.#node.flags?.collapsed === true, (collapsed) => {
            if (collapsed !== (this.#node.flags?.collapsed === true)) {
                this.#node.graph?.beforeChange?.();
                this.#node.collapse?.();
                this.#node.graph?.afterChange?.();
            }
            this.#refresh();
        }, { settle: () => settleNativeDisclosure(this.#target) });
    };
}
