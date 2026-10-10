import { NativeCardGeometryObserver } from '../NativeCardGeometryObserver.js';
import { ensureCubeSurfaceStyles } from '../CubeSurfaceStyles.js';
import { computeWorkflowMasonry } from './WorkflowMasonryLayout.js';
import { previewMasonrySections } from './WorkflowMasonryPreview.js';
import { WorkflowMasonrySectionView, } from './WorkflowMasonrySectionView.js';
import { SurfaceMotion } from '../motion/SurfaceMotion.js';
import { MasonryStackInsetMotion } from './MasonryStackInsetMotion.js';
import { SurfacePresentationBoundary } from '../motion/SurfacePresentationBoundary.js';
import { prepareNativePresentation } from '../NativePresentationReady.js';
import { measureNativeCardPresentationSources, restoreNativeCardPresentations, } from '../NativeCardPresentationOwner.js';
import { DISCLOSURE_LAYOUT_EVENT, measureDisclosureCardHeight, } from '../motion/DisclosureLayout.js';
/** Retain shared Cube presentation while measuring only masonry geometry. */
export class WorkflowMasonryView {
    element;
    backdrop;
    #content;
    #options;
    #views = new Map();
    #geometry;
    #motion = new SurfaceMotion();
    #stackInset = new MasonryStackInsetMotion();
    #presentation;
    #sections;
    #generation = 0;
    #disposed = false;
    #mounting = true;
    #transitioning = false;
    #pendingLayout = false;
    #searchCards = null;
    #previewCubeOrder = null;
    /** Prepare a continuous surface whose sections share existing Cube presentation owners. */
    constructor(options) {
        ensureCubeSurfaceStyles(options.document);
        this.#options = options;
        this.#presentation = new SurfacePresentationBoundary(options.document);
        this.#sections = options.sections;
        this.backdrop = options.document.createElement('div');
        this.backdrop.className = 'sugarcubes-workflow-masonry__backdrop';
        this.backdrop.style.opacity = '0';
        this.backdrop.setAttribute('aria-hidden', 'true');
        this.element = options.document.createElement('section');
        this.element.className = 'sugarcubes-workflow-masonry';
        this.element.style.opacity = '1';
        this.element.dataset.scMasonryPreparing = '';
        this.element.setAttribute('aria-label', 'Workflow masonry');
        // Comfy's ancestor capture handler checks this before forwarding wheel input to the graph.
        this.element.setAttribute('data-capture-wheel', 'true');
        this.element.tabIndex = 0;
        this.element.addEventListener('wheel', this.#onWheel, { passive: false });
        this.element.addEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
        this.#content = options.document.createElement('div');
        this.#content.className = 'sugarcubes-workflow-masonry__content';
        this.element.append(this.#content);
        this.#geometry = new NativeCardGeometryObserver(options.document, () => {
            if (!this.#mounting)
                this.#requestLayout();
        });
    }
    /** Read original source geometry before attaching the new surface or changing graph interaction. */
    prepareSources() {
        const sources = this.#options.getSources?.() ?? new Map();
        measureNativeCardPresentationSources(sources.values());
        return sources;
    }
    /** Mount native controls through the same path used for subsequent presentation changes. */
    mountCards(renderer, sources = this.prepareSources()) {
        prepareNativePresentation(this.element);
        return this.refresh(this.#sections, renderer, sources);
    }
    /** Retain section headers and open menus while reconciling card decisions in small batches. */
    async refresh(sections, renderer, sources = this.#options.getSources?.() ?? new Map()) {
        if (this.#disposed)
            return;
        const generation = ++this.#generation;
        this.#mounting = true;
        this.#sections = sections;
        this.#previewCubeOrder = null;
        const ids = new Set(sections.map((section) => section.id));
        for (const [id, view] of this.#views) {
            if (!ids.has(id)) {
                this.#motion.forget(view.header);
                view.dispose();
                this.#views.delete(id);
            }
        }
        const options = this.#layoutOptions();
        const pending = [];
        let projectionOrder = 0;
        for (const section of sections) {
            let view = this.#views.get(section.id);
            if (!view) {
                view = new WorkflowMasonrySectionView(section, {
                    content: this.#content,
                    cardState: this.#options.cardState,
                    openCube: this.#options.openCube,
                    motion: this.#motion,
                    getSource: (id) => this.#options.getSources?.().get(id) ?? null,
                    onRemounted: () => {
                        if (!this.#mounting)
                            this.#requestLayout();
                        this.#options.onCardsChanged?.();
                    },
                });
                this.#views.set(section.id, view);
            }
            pending.push(...view.prepare(section, renderer, sources));
            for (const card of section.cards) {
                const cell = view.cells.get(card.id);
                if (cell)
                    cell.dataset.scProjectionOrder = String(projectionOrder++);
            }
        }
        this.#content.querySelector('.sugarcubes-workflow-masonry__empty')?.remove();
        if (sections.length === 0) {
            const empty = this.#content.ownerDocument.createElement('p');
            empty.className = 'sugarcubes-workflow-masonry__empty';
            empty.textContent = 'Add nodes to your workflow to see them here.';
            this.#content.append(empty);
        }
        this.#assignWidths(computeWorkflowMasonry(sections.map((section) => ({
            id: section.id,
            cards: section.cards
                .filter((card) => card.visible)
                .map((card) => ({
                id: card.id,
                columnSpan: card.columnSpan,
                height: 36,
            })),
        })), options));
        let nativeMounts = 0;
        for (let index = 0; index < pending.length; index += 1) {
            if (this.#disposed || generation !== this.#generation)
                return;
            const item = pending[index];
            item?.mount();
            if (item?.requiresNativeMount)
                nativeMounts += 1;
            if (item?.requiresNativeMount && nativeMounts % 6 === 0 && index + 1 < pending.length) {
                await new Promise((resolve) => {
                    const windowRef = this.element.ownerDocument.defaultView;
                    if (windowRef?.requestAnimationFrame)
                        windowRef.requestAnimationFrame(() => resolve());
                    else
                        queueMicrotask(resolve);
                });
            }
        }
        if (this.#disposed || generation !== this.#generation)
            return;
        this.#mounting = false;
        this.#geometry.observe([
            this.element,
            ...[...this.#views.values()].flatMap((view) => [...view.cells.values()]),
        ]);
        this.#requestLayout();
    }
    /** Pack visible cards into section runs using one shared column width. */
    layout() {
        if (this.#disposed)
            return;
        if (this.#transitioning) {
            this.#pendingLayout = true;
            return;
        }
        const options = this.#layoutOptions();
        const measure = () => computeWorkflowMasonry(this.#layoutSections().map((section) => ({
            id: section.id,
            cards: section.cards
                .filter((card) => card.visible && this.#searchVisible(card.id))
                .map((card) => ({
                id: card.id,
                columnSpan: card.columnSpan,
                height: this.#measureCard(section.id, card.id),
            })),
        })), options);
        const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
        const initial = measure();
        const widthChanged = this.#assignWidths(initial);
        const result = widthChanged ? measure() : initial;
        const placements = [];
        for (const section of result.sections) {
            const header = this.#views.get(section.id)?.header;
            if (header) {
                header.dataset.masonryY = String(section.y);
                header.dataset.masonryHeight = String(section.height);
                placements.push({
                    element: header,
                    rectangle: {
                        x: 0,
                        y: section.y,
                        width: options.availableWidth,
                        height: 36,
                    },
                });
            }
        }
        for (const card of result.placements) {
            const cell = cells.get(card.id);
            if (cell)
                placements.push({ element: cell, rectangle: card });
        }
        this.#motion.place(placements);
        this.#motion.resizeHeight(this.#content, Math.max(result.height, 120));
        this.element.dispatchEvent(new CustomEvent('sc-masonry-layout'));
    }
    /** Expose the lifecycle-owned projection without rediscovering graph candidates from presentation events. */
    get sections() {
        return this.#sections;
    }
    /** Reserve rail space through ordinary layout so native controls animate rather than remount. */
    setStackWidth(width) {
        if (!Number.isFinite(width) || width < 0)
            throw new TypeError('Cube stack width must be finite and nonnegative.');
        this.#stackInset.set(this.element, this.#content, width, () => this.#requestLayout());
    }
    /** Preview an exact section permutation through existing compositor motion without editing workflow state. */
    previewCubeOrder(sectionIds) {
        previewMasonrySections(this.#sections, sectionIds);
        this.#previewCubeOrder = sectionIds === null ? null : [...sectionIds];
        this.#requestLayout();
    }
    /** Accept canonical reading order while retaining every mounted section and native control. */
    setCubeOrder(sectionIds) {
        const current = this.#sections
            .filter((section) => section.cube !== null)
            .map((section) => section.id);
        if (current.every((id, index) => id === sectionIds[index]) &&
            current.length === sectionIds.length)
            return;
        this.#sections = [...previewMasonrySections(this.#sections, sectionIds)];
        this.#requestLayout();
    }
    /** Return identified mounted cells for scoped native-field adapters. */
    cards() {
        return new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
    }
    /** Pack finder-visible cards without changing persistent node visibility or native mount identity. */
    setSearchCards(cards, remeasure = false) {
        const same = cards === this.#searchCards ||
            (cards !== null &&
                this.#searchCards !== null &&
                cards.size === this.#searchCards.size &&
                [...cards].every((id) => this.#searchCards?.has(id)));
        if (same && !remeasure)
            return;
        this.#searchCards = cards;
        for (const view of this.#views.values()) {
            let visible = false;
            for (const [id, cell] of view.cells) {
                const shown = this.#searchVisible(id);
                cell.toggleAttribute('data-sc-search-card-hidden', !shown);
                visible ||= shown;
            }
            view.header.toggleAttribute('data-sc-search-card-hidden', cards !== null && !visible);
        }
        this.#requestLayout();
    }
    /** Keep empty filtered sections out of packing while retaining their actual mounted headers. */
    #layoutSections() {
        const sections = previewMasonrySections(this.#sections, this.#previewCubeOrder);
        return this.#searchCards === null
            ? sections
            : sections.filter((section) => section.cards.some((card) => card.visible && this.#searchVisible(card.id)));
    }
    /** Resolve the transient finder mask independently of policy visibility. */
    #searchVisible(id) {
        return this.#searchCards === null || this.#searchCards.has(id);
    }
    /** Read container constraints once before any card width writes. */
    #layoutOptions() {
        return {
            availableWidth: Math.max(1, this.#content.clientWidth),
            minimumColumnWidth: Number.parseFloat(getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-column-width')) || 320,
            gap: Number.parseFloat(getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-gap')) || 12,
        };
    }
    /** Establish native control widths before mount so text and sliders measure their final columns. */
    #assignWidths(layout) {
        const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
        let changed = false;
        for (const card of layout.placements) {
            const cell = cells.get(card.id);
            const width = `${String(card.width)}px`;
            if (cell && cell.style.width !== width) {
                cell.style.width = width;
                changed = true;
            }
        }
        return changed;
    }
    /** Use settled disclosure geometry so neighbors share one smooth destination transition. */
    #measureCard(sectionId, cardId) {
        const cell = this.#views.get(sectionId)?.cells.get(cardId);
        return cell ? measureDisclosureCardHeight(cell) : 36;
    }
    /** Pack simultaneous disclosure destinations together in the existing geometry frame. */
    #onDisclosureLayout = () => {
        if (!this.#mounting)
            this.#geometry.request();
    };
    /** Coalesce initial native styling into the single endpoint layout that starts assembly. */
    #requestLayout() {
        if (this.element.hasAttribute('data-sc-masonry-preparing'))
            this.#pendingLayout = true;
        else
            this.layout();
    }
    /** Freeze scroll coordinates during card travel and keep wheel input inside the native surface. */
    #onWheel = (event) => {
        if (this.#transitioning)
            event.preventDefault();
        event.stopPropagation();
    };
    /** Navigate by stable section identity for shared Cube stack navigation. */
    scrollToSection(id) {
        const header = this.#views.get(id)?.header;
        if (header)
            this.element.scrollTo({
                top: header.offsetTop + Number(header.dataset.masonryY ?? 0),
                behavior: this.#options.document.defaultView?.matchMedia?.('(prefers-reduced-motion: reduce)').matches
                    ? 'instant'
                    : 'smooth',
            });
    }
    /** Expose complete painted cells after SurfaceMotion commits their settled placements. */
    transitionCards() {
        return [...this.#views.values()].flatMap((view) => [...view.cells].flatMap(([id, cell]) => {
            return cell.querySelector('.sugarcubes-cube-face__native-card-mount') ? [{ id, cell }] : [];
        }));
    }
    /** Wait for native mount or restored graph styles before measuring animation endpoints. */
    settlePresentation() {
        if (this.element.hasAttribute('data-sc-masonry-preparing')) {
            prepareNativePresentation(this.element);
            return this.#presentation.settled(1);
        }
        return this.#stackInset.settled().then(() => {
            if (!this.#disposed)
                return this.#presentation.settled();
        });
    }
    /** Hold settled endpoints during assembly and reconcile any queued resize afterward. */
    setTransitioning(transitioning) {
        if (transitioning && !this.#transitioning) {
            this.element.scrollTo?.({ top: this.element.scrollTop, behavior: 'instant' });
            if (this.#pendingLayout || this.element.hasAttribute('data-sc-masonry-preparing')) {
                this.#pendingLayout = false;
                this.layout();
            }
            this.#motion.finishAnimations();
        }
        this.#transitioning = transitioning;
        if (!transitioning)
            this.#motion.settle();
        if (!transitioning && this.#pendingLayout) {
            this.#pendingLayout = false;
            this.layout();
        }
    }
    /** Release observers and native mounts before restoring singleton custom widgets. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.element.removeEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
        this.element.removeEventListener('wheel', this.#onWheel);
        this.#generation += 1;
        this.#geometry.dispose();
        this.#presentation.cancel();
        this.#motion.dispose();
        this.#stackInset.dispose();
        restoreNativeCardPresentations(this.element.querySelectorAll('.sugarcubes-cube-face__node-card'));
        for (const view of this.#views.values())
            view.dispose();
        this.#views.clear();
        this.element.remove();
        this.backdrop.remove();
    }
}
