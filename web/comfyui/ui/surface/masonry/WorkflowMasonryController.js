import { WorkflowMasonryView } from './WorkflowMasonryView.js';
import { WorkflowMasonryTransition } from './WorkflowMasonryTransition.js';
/** Keep layout choice transient and restore graph presentation on every failure path. */
export class WorkflowMasonryController {
    #options;
    #transition = new WorkflowMasonryTransition();
    #view = null;
    #container = null;
    #desired = false;
    #generation = 0;
    #mounting = null;
    #topology = '';
    #renderer = null;
    /** Bind host capabilities without importing host globals or owning graph mutations. */
    constructor(options) {
        this.#options = options;
    }
    /** Expose the requested presentation mode for button and future stack navigation. */
    get active() {
        return this.#desired;
    }
    /** Reverse the presentation with one authoritative state transition. */
    async toggle() {
        if (!this.#options.isNodes2())
            return;
        this.#desired = !this.#desired;
        const generation = ++this.#generation;
        this.#options.onStateChange();
        if (!this.#desired) {
            this.#setGraphHidden(false);
            await this.#animate(false);
            if (generation === this.#generation)
                this.#releaseView();
            return;
        }
        try {
            if (!this.#view) {
                const renderer = await this.#options.getRenderer();
                if (generation !== this.#generation || !this.#desired || !this.#options.isNodes2())
                    return;
                const container = this.#options.getContainer();
                if (!container)
                    throw new Error('Comfy graph container is not mounted.');
                const sections = this.#options.getSections();
                this.#renderer = renderer;
                this.#topology = topologySignature(sections);
                this.#view = new WorkflowMasonryView({
                    document: this.#options.document,
                    sections,
                    cardState: this.#options.getCardState(),
                    openCube: (cube) => {
                        this.leaveImmediately();
                        this.#options.openCube(cube);
                    },
                });
                this.#container = container;
                this.#view.element.style.opacity = '0';
                container.append(this.#view.element);
                this.#view.layout();
                this.#mounting = this.#view.mountCards(renderer);
            }
            await this.#mounting;
            if (generation !== this.#generation || !this.#desired)
                return;
            await this.#animate(true);
            if (generation === this.#generation && this.#desired)
                this.#setGraphHidden(true);
        }
        catch (error) {
            if (generation !== this.#generation)
                return;
            this.leaveImmediately();
            this.#options.reportError(error);
        }
    }
    /** Restore graph immediately before graph replacement, renderer teardown, or application disposal. */
    leaveImmediately() {
        this.#desired = false;
        this.#generation += 1;
        this.#transition.cancel();
        this.#releaseView();
        this.#options.onStateChange();
    }
    /** Explain the unsupported renderer once, only when it interrupts active masonry. */
    rendererChanged() {
        const unsupported = !this.#options.isNodes2();
        if (unsupported && (this.#desired || this.#view)) {
            this.leaveImmediately();
            void this.#options
                .alert('Masonry requires Nodes 2.0', 'SugarCubes masonry does not support Nodes 1.0. Your workflow is now displayed in graph mode.')
                .catch((error) => this.#options.reportError(error));
        }
        this.#options.onStateChange();
    }
    /** Address sections by identity so a Cube stack can share this navigation surface. */
    scrollToSection(id) {
        this.#view?.scrollToSection(id);
    }
    /** Reconcile card presentation without replaying transitions or closing the optional-node menu. */
    async graphChanged() {
        if (!this.#desired || !this.#view || !this.#renderer)
            return;
        await this.#mounting;
        if (!this.#desired || !this.#view)
            return;
        const sections = this.#options.getSections();
        const topology = topologySignature(sections);
        if (this.#topology === topology)
            return;
        this.#topology = topology;
        try {
            this.#mounting = this.#view.refresh(sections, this.#renderer);
            await this.#mounting;
        }
        catch (error) {
            this.leaveImmediately();
            this.#options.reportError(error);
        }
    }
    /** Fade the complete surface without reading or animating node geometry. */
    async #animate(entering) {
        const view = this.#view;
        if (!view)
            return;
        const windowRef = this.#options.document.defaultView;
        await this.#transition.run(view.element, entering, windowRef?.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false);
    }
    /** Remove presentation resources while preserving canonical graph and edit state. */
    #releaseView() {
        this.#setGraphHidden(false);
        this.#view?.dispose();
        this.#view = null;
        this.#mounting = null;
        this.#renderer = null;
        this.#container = null;
    }
    /** Keep graph visible behind the fade and suppress it only after masonry is opaque. */
    #setGraphHidden(hidden) {
        const container = this.#container;
        if (!container)
            return;
        const graphRoot = container.closest('#graph-canvas-container');
        for (const element of new Set([container, graphRoot])) {
            if (!element)
                continue;
            if (hidden)
                element.dataset.sugarcubesMasonry = 'true';
            else
                delete element.dataset.sugarcubesMasonry;
        }
    }
}
/** Fingerprint presentation membership without treating widget edits as remount requests. */
function topologySignature(sections) {
    return JSON.stringify(sections.map((section) => [
        section.id,
        section.label,
        section.cards.map((card) => [
            card.id,
            card.node.type,
            card.label,
            card.columnSpan,
            card.visible,
            card.enabled,
            card.showActivationControl,
        ]),
    ]));
}
