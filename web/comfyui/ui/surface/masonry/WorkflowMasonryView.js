import { NativeCardGeometryObserver } from '../NativeCardGeometryObserver.js';
import { ensureCubeSurfaceStyles } from '../CubeSurfaceStyles.js';
import { computeWorkflowMasonry } from './WorkflowMasonryLayout.js';
import { WorkflowMasonrySectionView } from './WorkflowMasonrySectionView.js';
import { SurfaceMotion } from '../motion/SurfaceMotion.js';
import { DISCLOSURE_LAYOUT_EVENT, measureDisclosureCardHeight, } from '../motion/DisclosureLayout.js';
/** Retain shared Cube presentation while measuring only masonry geometry. */
export class WorkflowMasonryView {
    element;
    #content;
    #options;
    #views = new Map();
    #geometry;
    #motion = new SurfaceMotion();
    #sections;
    #generation = 0;
    #disposed = false;
    #mounting = true;
    /** Prepare a continuous surface whose sections share existing Cube presentation owners. */
    constructor(options) {
        ensureCubeSurfaceStyles(options.document);
        this.#options = options;
        this.#sections = options.sections;
        this.element = options.document.createElement('section');
        this.element.className = 'sugarcubes-workflow-masonry';
        this.element.setAttribute('aria-label', 'Workflow masonry');
        // Comfy's ancestor capture handler checks this before forwarding wheel input to the graph.
        this.element.setAttribute('data-capture-wheel', 'true');
        this.element.tabIndex = 0;
        this.element.addEventListener('wheel', (event) => event.stopPropagation(), { passive: true });
        this.element.addEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
        this.#content = options.document.createElement('div');
        this.#content.className = 'sugarcubes-workflow-masonry__content';
        this.element.append(this.#content);
        this.#geometry = new NativeCardGeometryObserver(options.document, () => {
            if (!this.#mounting)
                this.layout();
        });
    }
    /** Mount native controls through the same path used for subsequent presentation changes. */
    mountCards(renderer) {
        return this.refresh(this.#sections, renderer);
    }
    /** Retain section headers and open menus while reconciling card decisions in small batches. */
    async refresh(sections, renderer) {
        if (this.#disposed)
            return;
        const generation = ++this.#generation;
        this.#mounting = true;
        this.#sections = sections;
        const ids = new Set(sections.map((section) => section.id));
        for (const [id, view] of this.#views) {
            if (!ids.has(id)) {
                this.#motion.forget(view.header);
                view.dispose();
                this.#views.delete(id);
            }
        }
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
                });
                this.#views.set(section.id, view);
            }
            pending.push(...view.prepare(section, renderer));
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
        for (let index = 0; index < pending.length; index += 1) {
            if (this.#disposed || generation !== this.#generation)
                return;
            pending[index]?.();
            if ((index + 1) % 6 === 0 && index + 1 < pending.length) {
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
        this.layout();
    }
    /** Pack visible cards into section runs using one shared column width. */
    layout() {
        if (this.#disposed)
            return;
        const options = {
            availableWidth: Math.max(1, this.#content.clientWidth),
            minimumColumnWidth: Number.parseFloat(getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-column-width')) || 320,
            gap: Number.parseFloat(getComputedStyle(this.element).getPropertyValue('--sugarcubes-masonry-gap')) || 12,
        };
        const measure = () => computeWorkflowMasonry(this.#sections.map((section) => ({
            id: section.id,
            cards: section.cards
                .filter((card) => card.visible)
                .map((card) => ({
                id: card.id,
                columnSpan: card.columnSpan,
                height: this.#measureCard(section.id, card.id),
            })),
        })), options);
        const cells = new Map([...this.#views.values()].flatMap((view) => [...view.cells]));
        for (const card of measure().placements) {
            const cell = cells.get(card.id);
            if (cell && cell.style.width !== `${String(card.width)}px`)
                cell.style.width = `${String(card.width)}px`;
        }
        const result = measure();
        for (const section of result.sections) {
            const header = this.#views.get(section.id)?.header;
            if (header) {
                header.dataset.masonryY = String(section.y);
                this.#motion.place(header, {
                    x: 0,
                    y: section.y,
                    width: options.availableWidth,
                    height: 36,
                });
            }
        }
        for (const card of result.placements) {
            const cell = cells.get(card.id);
            if (cell)
                this.#motion.place(cell, card);
        }
        this.#motion.settle();
        this.#motion.resizeHeight(this.#content, Math.max(result.height, 120));
    }
    /** Use settled disclosure geometry so neighbors share one smooth destination transition. */
    #measureCard(sectionId, cardId) {
        const cell = this.#views.get(sectionId)?.cells.get(cardId);
        return cell ? measureDisclosureCardHeight(cell) : 36;
    }
    /** Reflow at disclosure start and release, retaining native resize observation for other changes. */
    #onDisclosureLayout = () => {
        if (!this.#mounting)
            this.layout();
    };
    /** Navigate by stable section identity for shared Cube stack navigation. */
    scrollToSection(id) {
        const header = this.#views.get(id)?.header;
        if (header)
            this.element.scrollTo({
                top: header.offsetTop + Number(header.dataset.masonryY ?? 0),
                behavior: 'smooth',
            });
    }
    /** Release observers and native mounts before restoring singleton custom widgets. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.element.removeEventListener(DISCLOSURE_LAYOUT_EVENT, this.#onDisclosureLayout);
        this.#generation += 1;
        this.#geometry.dispose();
        this.#motion.dispose();
        for (const view of this.#views.values())
            view.dispose();
        this.#views.clear();
        this.element.remove();
    }
}
