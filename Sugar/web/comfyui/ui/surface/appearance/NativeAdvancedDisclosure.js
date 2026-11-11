import { NativeGraphAdvancedDisclosure } from './NativeGraphAdvancedDisclosure.js';
import { NativeProjectedAdvancedDisclosure } from './NativeProjectedAdvancedDisclosure.js';
import { readProjectedAdvancedInputMount } from '../NativeProjectedAdvancedInputs.js';
/** Route presentation to the mounted state owner rather than competing click handlers. */
export class NativeAdvancedDisclosure {
    #buttons = new Map();
    #graph = new NativeGraphAdvancedDisclosure();
    #projected = new NativeProjectedAdvancedDisclosure();
    /** Reconcile each recognized card through its renderer's disclosure lifetime. */
    reconcile(card) {
        for (const [button, attributes] of this.#buttons) {
            if (!button.isConnected) {
                restoreButton(button, attributes);
                this.#buttons.delete(button);
            }
        }
        if (card.advanced) {
            const { button, expanded } = card.advanced;
            if (!this.#buttons.has(button))
                this.#buttons.set(button, new Map(['aria-label', 'aria-expanded'].map((name) => [name, button.getAttribute(name)])));
            button.setAttribute('aria-label', `${expanded ? 'Hide' : 'Show'} advanced inputs`);
            button.setAttribute('aria-expanded', String(expanded));
        }
        const projection = readProjectedAdvancedInputMount(card.root);
        if (projection)
            this.#projected.reconcile(projection, card);
        else
            this.#graph.reconcile(card);
    }
    /** Release both presentations while retaining the latest native advanced state. */
    clear() {
        this.#graph.clear();
        this.#projected.clear();
        for (const [button, attributes] of this.#buttons)
            restoreButton(button, attributes);
        this.#buttons.clear();
    }
}
/** Return host accessibility attributes when themed disclosure presentation retires. */
function restoreButton(button, attributes) {
    for (const [name, value] of attributes) {
        if (value === null)
            button.removeAttribute(name);
        else
            button.setAttribute(name, value);
    }
}
