import { isRecord } from '../../types/common.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { PROMPT_VALUE_CHANGE_EVENT } from '../../surface/CubePromptTextareaAutosizer.js';
import { subscribeNativeNodeCardRefresh } from '../../surface/NativeNodeCardRefresh.js';
import { MasonrySearchPresenter } from '../presentation/MasonrySearchPresenter.js';
import { MasonrySearchCorpus } from './MasonrySearchCorpus.js';
/** Retain only portable finder state between surfaces, keyed weakly to the host workflow lifetime. */
export class MasonrySearchHost {
    #options;
    #states = new WeakMap();
    #scope = null;
    #view = null;
    #presenter = null;
    #corpus = null;
    #presence = null;
    #unsubscribeValues = null;
    #frame = null;
    #dirtyCards = new Set();
    #started = false;
    /** Bind host capabilities without discovering graph candidates during finder interaction. */
    constructor(options) {
        this.#options = options;
    }
    /** Register Ctrl+F outside graph event capture while respecting modal keyboard ownership. */
    setup() {
        if (this.#started)
            return;
        this.#started = true;
        this.#options.document.defaultView?.addEventListener('keydown', this.#shortcut, true);
    }
    /** Attach the finder only to a mounted authoritative masonry projection. */
    ready(view) {
        if (this.#view !== view)
            this.release();
        const graph = this.#options.getGraph();
        const container = view.element.parentElement;
        if (!graph || !container)
            return;
        const corpus = new MasonrySearchCorpus(view.sections, view.cards());
        this.#corpus = corpus;
        this.#unsubscribeValues?.();
        const nodes = view.sections.flatMap((section) => section.cards.flatMap((card) => [
            card.node,
            ...(card.widgetSources ?? []).map((source) => source.node),
        ]));
        this.#unsubscribeValues = subscribeNativeNodeCardRefresh(this.#options.document, nodes, () => this.valuesChanged());
        if (this.#presenter && this.#view === view) {
            this.#presenter.updateSurface(corpus);
            return;
        }
        this.#view = view;
        this.#scope = {
            owner: workflowOwner(this.#options.app, graph),
            graph: String(graph.id ?? 'root'),
        };
        const state = this.#states.get(this.#scope.owner)?.get(this.#scope.graph);
        this.#presenter = new MasonrySearchPresenter({
            root: view.element,
            container,
            surface: corpus,
            ...(state ? { state } : {}),
            setCards: (nodes, remeasure) => view.setSearchCards(nodes, remeasure),
        });
        view.element.addEventListener('input', this.#input);
        view.element.addEventListener('change', this.#input);
        view.element.addEventListener(PROMPT_VALUE_CHANGE_EVENT, this.#input);
        this.#presence = new HostSurfaceObserver(view.element, '[data-testid="node-widget"]', (changes) => {
            for (const change of changes) {
                if (change.relocated)
                    continue;
                const id = this.#corpus?.cardFor(change.element);
                if (id)
                    this.#dirtyCards.add(id);
            }
            if (this.#dirtyCards.size)
                this.valuesChanged();
        }, { exclude: '.sc-search-highlight' });
    }
    /** Coalesce scoped native value changes without running candidate discovery or replacing the finder. */
    valuesChanged() {
        if (this.#frame !== null || !this.#presenter)
            return;
        const windowRef = this.#options.document.defaultView;
        if (!windowRef)
            return;
        this.#frame = windowRef.requestAnimationFrame(() => {
            this.#frame = null;
            try {
                if (this.#dirtyCards.size && this.#corpus) {
                    this.#corpus.refreshCards(this.#dirtyCards);
                    this.#dirtyCards.clear();
                    this.#presenter?.updateSurface(this.#corpus);
                }
                else
                    this.#presenter?.valuesChanged();
            }
            catch (error) {
                this.#options.reportError(error);
            }
        });
    }
    /** Reveal the full projection before navigation from the stack, retaining the user's query. */
    revealAll() {
        this.#presenter?.close(false);
    }
    /** Save query and location before releasing every mounted control, highlight and observer. */
    release() {
        if (this.#frame !== null)
            this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.#presence?.disconnect();
        this.#presence = null;
        this.#unsubscribeValues?.();
        this.#unsubscribeValues = null;
        const root = this.#view?.element;
        root?.removeEventListener('input', this.#input);
        root?.removeEventListener('change', this.#input);
        root?.removeEventListener(PROMPT_VALUE_CHANGE_EVENT, this.#input);
        if (this.#presenter && this.#scope) {
            const states = this.#states.get(this.#scope.owner) ?? new Map();
            states.set(this.#scope.graph, this.#presenter.state);
            this.#states.set(this.#scope.owner, states);
        }
        this.#presenter?.dispose();
        this.#presenter = null;
        this.#corpus = null;
        this.#view = null;
        this.#scope = null;
        this.#dirtyCards.clear();
    }
    /** Restore keyboard ownership when the extension or document is disposed. */
    dispose() {
        this.release();
        this.#options.document.defaultView?.removeEventListener('keydown', this.#shortcut, true);
        this.#started = false;
    }
    /** Ignore presentation-only input and match only fields already owned by this surface. */
    #input = (event) => {
        if (!(event.target instanceof HTMLElement))
            return;
        const row = event.target.closest('[data-testid="node-widget"]');
        if (row && this.#corpus?.cardFor(row))
            this.valuesChanged();
    };
    /** Open and select the retained query only while masonry owns the viewport. */
    #shortcut = (event) => {
        if (!this.#presenter ||
            !event.ctrlKey ||
            event.altKey ||
            event.key.toLowerCase() !== 'f' ||
            [...this.#options.document.querySelectorAll('[role="dialog"][aria-modal="true"]')].some((dialog) => dialog.checkVisibility({ visibilityProperty: true })))
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        try {
            this.#presenter.open();
        }
        catch (error) {
            this.#options.reportError(error);
        }
    };
}
/** Use the host's workflow object as a weak lifetime key, falling back to the native graph boundary. */
function workflowOwner(app, graph) {
    const manager = isRecord(app?.extensionManager) ? app.extensionManager : null;
    const workflow = isRecord(manager?.workflow) ? manager.workflow : null;
    return isRecord(workflow?.activeWorkflow) ? workflow.activeWorkflow : graph;
}
