import { createCubeModelTitleElement } from '../surface/CubeModelPillDomRenderer.js';
/** Retain host children and reconcile rich titles without disturbing native menu identity. */
export class CubeEditorTitlePresenter {
    #leases = new Map();
    /** Replace only changed semantic titles and recover if the host overwrites owned content. */
    present(host, presentation) {
        const previous = this.#leases.get(host);
        const signature = JSON.stringify(presentation);
        if (previous?.signature === signature && host.firstChild === previous.title)
            return;
        const title = createCubeModelTitleElement(host.ownerDocument, presentation);
        this.#leases.set(host, {
            children: previous?.children ?? [...host.childNodes],
            title,
            signature,
        });
        host.replaceChildren(title);
    }
    /** Forget unmounted host surfaces without retaining their borrowed child nodes. */
    prune() {
        for (const host of this.#leases.keys())
            if (!host.isConnected)
                this.#leases.delete(host);
    }
    /** Restore the exact borrowed host children when the Cube context ends. */
    clear() {
        for (const [host, lease] of this.#leases)
            host.replaceChildren(...lease.children);
        this.#leases.clear();
    }
}
