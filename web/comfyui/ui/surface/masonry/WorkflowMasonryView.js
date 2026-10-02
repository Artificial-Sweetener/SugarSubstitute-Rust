import { NativeCardGeometryObserver } from '../NativeCardGeometryObserver.js';
import { ensureCubeSurfaceStyles } from '../CubeSurfaceStyles.js';
import { computeWorkflowMasonry } from './WorkflowMasonryLayout.js';
import { WorkflowMasonrySectionView } from './WorkflowMasonrySectionView.js';
/** Retain shared Cube presentation while measuring only masonry geometry. */
export class WorkflowMasonryView {
    element;
    #content;
    #options;
    #views = new Map();
    #geometry;
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
        this.element.tabIndex = 0;
        this.element.addEventListener('wheel', (event) => event.stopPropagation(), { passive: true });
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
                view.dispose();
                this.#views.delete(id);
            }
        }
        const pending = [];
        for (const section of sections) {
            let view = this.#views.get(section.id);
            if (!view) {
                view = new WorkflowMasonrySectionView(section, {
                    content: this.#content,
                    cardState: this.#options.cardState,
                    openCube: this.#options.openCube,
                });
                this.#views.set(section.id, view);
            }
            pending.push(...view.prepare(section, renderer));
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
            minimumColumnWidth: 320,
            gap: 12,
        };
        const measure = () => computeWorkflowMasonry(this.#sections.map((section) => ({
            id: section.id,
            cards: section.cards
                .filter((card) => card.visible)
                .map((card) => ({
                id: card.id,
                columnSpan: card.columnSpan,
                height: this.#views.get(section.id)?.cells.get(card.id)?.offsetHeight ?? 36,
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
            if (header)
                header.style.transform = `translateY(${String(section.y)}px)`;
        }
        for (const card of result.placements) {
            const cell = cells.get(card.id);
            if (cell)
                cell.style.transform = `translate(${String(card.x)}px, ${String(card.y)}px)`;
        }
        this.#content.style.height = `${String(Math.max(result.height, 120))}px`;
    }
    /** Navigate by stable section identity for shared Cube stack navigation. */
    scrollToSection(id) {
        const header = this.#views.get(id)?.header;
        if (header)
            this.element.scrollTo({
                top: header.offsetTop + Number(header.style.transform.match(/[-\d.]+/)?.[0] ?? 0),
                behavior: 'smooth',
            });
    }
    /** Release observers and native mounts before restoring singleton custom widgets. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#generation += 1;
        this.#geometry.dispose();
        for (const view of this.#views.values())
            view.dispose();
        this.#views.clear();
        this.element.remove();
    }
}
