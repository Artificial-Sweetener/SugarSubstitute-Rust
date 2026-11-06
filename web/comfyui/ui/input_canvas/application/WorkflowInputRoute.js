import { workflowMaskKey } from '../domain/WorkflowSurfacePlan.js';
/** A workflow that remembers no selection opens on its input images, as it did in Substitute. */
const STARTING = { route: 'input', surface: null, mask: null };
/**
 * Keep the canvas's visible route and input document in step with the remembered selection.
 *
 * Input is available only while the workflow has surfaces; otherwise the view stays on Output.
 * A workflow without a remembered selection starts on Input when it has surfaces.
 * A remembered surface or mask that disappears is cleared rather than reassigned. Showing Input
 * with no active surface activates the first surface and its first mask.
 *
 * A card that opens Input also asks the canvas to take keyboard focus, so its editing keys apply
 * at once. A newly chosen image opens once provisioning holds its surface.
 */
export class WorkflowInputRoute {
    surfaces;
    host;
    store;
    report;
    #listeners = new Set();
    #focusListeners = new Set();
    /** An image endpoint whose newly chosen file opens on Input once its surface is provisioned. */
    #opening = null;
    #stop;
    #workflow = null;
    #selection = STARTING;
    #view = null;
    #activated = null;
    /** Follow synced surfaces and send activation through the canvas that holds them. */
    constructor(surfaces, host, store, report) {
        this.surfaces = surfaces;
        this.host = host;
        this.store = store;
        this.report = report;
        this.#stop = surfaces.subscribe(() => this.#reconcile());
        this.#reconcile();
    }
    /** The route view, or null when no workflow-surface canvas holds this workflow. */
    read() {
        return this.#view;
    }
    /** Observe route view changes. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Observe requests for the canvas to take keyboard focus after a card opened Input. */
    subscribeFocus(listener) {
        this.#focusListeners.add(listener);
        return () => {
            this.#focusListeners.delete(listener);
        };
    }
    /** Show Input or Output; Input is refused while the workflow has no surfaces. */
    chooseRoute(route) {
        const state = this.surfaces.state;
        if (!state || (route === 'input' && !state.surfaces.length))
            return;
        this.#choose({ ...this.#selection, route });
    }
    /** Show an image endpoint's surface with its first mask on the Input route, with focus. */
    activateImage(endpoint) {
        const surface = this.surfaces.state?.surfaces.find((item) => item.image === endpoint);
        if (!surface)
            return false;
        this.#choose({ route: 'input', surface: surface.key, mask: surface.masks[0]?.key ?? null });
        this.#focus();
        return true;
    }
    /**
     * Open the image the user just chose for an endpoint on the Input route, with focus.
     *
     * An endpoint that had no file has no surface until provisioning syncs the new one, so the
     * request waits for that surface. Another choice or a workflow switch abandons it.
     */
    openImage(endpoint) {
        if (this.activateImage(endpoint))
            return;
        const workflow = this.#workflow;
        this.#opening = workflow ? { workflow, endpoint } : null;
    }
    /** Show a mask's surface, then make that mask active, on the Input route. */
    activateMask(endpoint, entry) {
        const key = workflowMaskKey(endpoint, entry);
        const surface = this.surfaces.state?.surfaces.find((item) => item.masks.some((mask) => mask.key === key || (entry === null && mask.endpoint === endpoint)));
        const mask = surface?.masks.find((item) => item.key === key || (entry === null && item.endpoint === endpoint));
        if (!surface || !mask)
            return false;
        this.#choose({ route: 'input', surface: surface.key, mask: mask.key });
        this.#focus();
        return true;
    }
    /** Stop following surfaces. */
    dispose() {
        this.#stop();
        this.#listeners.clear();
        this.#focusListeners.clear();
    }
    /** Remember a user's choice for this workflow and apply it. */
    #choose(selection) {
        const workflow = this.#workflow;
        if (!workflow)
            return;
        this.#opening = null;
        this.#selection = selection;
        try {
            this.store.write(workflow, selection);
        }
        catch (error) {
            this.report(error);
        }
        this.#reconcile();
    }
    /** Validate the selection against synced surfaces, publish the view and activate on change. */
    #reconcile() {
        const state = this.surfaces.state;
        if (!state) {
            this.#workflow = null;
            this.#activated = null;
            this.#publish(null);
            return;
        }
        if (state.workflow !== this.#workflow) {
            this.#workflow = state.workflow;
            this.#activated = null;
            try {
                this.#selection = this.store.read(state.workflow) ?? STARTING;
            }
            catch (error) {
                this.report(error);
                this.#selection = STARTING;
            }
        }
        if (this.#open(state))
            return;
        const available = state.surfaces.length > 0;
        let surface = state.surfaces.find((item) => item.key === this.#selection.surface) ?? null;
        let mask = surface?.masks.find((item) => item.key === this.#selection.mask)?.key ?? null;
        const route = available ? this.#selection.route : 'output';
        if (route === 'input' && !surface) {
            surface = state.surfaces[0] ?? null;
            mask = surface?.masks[0]?.key ?? null;
        }
        this.#selection = { route: this.#selection.route, surface: surface?.key ?? null, mask };
        this.#publish({
            route,
            surface: surface?.key ?? null,
            mask,
            available,
        });
        const command = {
            type: 'activate',
            workflow: state.workflow,
            route,
            surface: surface?.key ?? null,
            mask,
        };
        const signature = JSON.stringify([state.canvasId, command]);
        if (signature === this.#activated)
            return;
        const host = this.host();
        if (!host || host.canvasId !== state.canvasId)
            return;
        this.#activated = signature;
        host.execute(command).then((result) => {
            if (result.kind === 'failure') {
                if (this.#activated === signature)
                    this.#activated = null;
                if (result.status !== 'cancelled')
                    this.report(new Error(result.message));
            }
        }, (error) => {
            if (this.#activated === signature)
                this.#activated = null;
            this.report(error);
        });
    }
    /** Open a waiting chosen image once its surface exists; reports whether it reconciled. */
    #open(state) {
        const opening = this.#opening;
        if (!opening)
            return false;
        if (opening.workflow !== state.workflow) {
            this.#opening = null;
            return false;
        }
        if (!state.surfaces.some((item) => item.image === opening.endpoint))
            return false;
        return this.activateImage(opening.endpoint);
    }
    /** Ask the canvas to take keyboard focus. */
    #focus() {
        for (const listener of this.#focusListeners)
            try {
                listener();
            }
            catch (error) {
                this.report(error);
            }
    }
    /** Publish a changed view. */
    #publish(view) {
        if (JSON.stringify(view) === JSON.stringify(this.#view))
            return;
        this.#view = view;
        for (const listener of this.#listeners)
            try {
                listener();
            }
            catch (error) {
                this.report(error);
            }
    }
}
