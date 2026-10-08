import { NativeGraphAdvancedDisclosure } from './NativeGraphAdvancedDisclosure.js';
import { NativeProjectedAdvancedDisclosure } from './NativeProjectedAdvancedDisclosure.js';
import { readProjectedAdvancedInputMount } from '../NativeProjectedAdvancedInputs.js';
/** Route presentation to the mounted state owner rather than competing click handlers. */
export class NativeAdvancedDisclosure {
    #graph = new NativeGraphAdvancedDisclosure();
    #projected = new NativeProjectedAdvancedDisclosure();
    /** Reconcile each recognized card through its renderer's disclosure lifetime. */
    reconcile(card) {
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
    }
}
