import { settleNativeDisclosure } from '../NativeDisclosureSettlement.js';
import { measureProjectedAdvancedHeight } from './NativeAdvancedDisclosureGeometry.js';
/** Retain one animation on the stable mount and delegate all native changes to the exact-node owner. */
export class NativeProjectedAdvancedDisclosure {
    #bindings = new Map();
    #releasing = new Set();
    /** Preserve in-flight presentation when the native root and its widgets are replaced. */
    reconcile(mount, card) {
        if (this.#releasing.has(mount.target))
            return;
        if (!card.advanced || !card.root.hasAttribute('data-sc-style-node')) {
            this.#release(mount.target);
            return;
        }
        if (this.#bindings.has(mount.target))
            return;
        const motion = mount.motion;
        const unsubscribe = mount.present(() => motion.toggle(!mount.control.shown(), (collapsed) => mount.control.setShown(!collapsed), {
            contentLayout: 'fill',
            closedHeight: () => measureProjectedAdvancedHeight(mount.target, true),
            expandedHeight: () => measureProjectedAdvancedHeight(mount.target, false),
            settle: () => settleNativeDisclosure(mount.target),
        }, mount.control), () => this.#release(mount.target));
        this.#bindings.set(mount.target, { motion, unsubscribe });
    }
    /** Commit the latest request and detach every projected presentation when themed ends. */
    clear() {
        for (const target of [...this.#bindings.keys()])
            this.#release(target);
    }
    /** Release before committing state so a synchronous remount cannot reinstall a disposed binding. */
    #release(target) {
        const binding = this.#bindings.get(target);
        if (!binding)
            return;
        this.#bindings.delete(target);
        this.#releasing.add(target);
        try {
            binding.unsubscribe();
            binding.motion.dispose();
        }
        finally {
            this.#releasing.delete(target);
        }
    }
}
