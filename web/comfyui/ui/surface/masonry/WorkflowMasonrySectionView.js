import { WorkflowMasonryCardMount } from './WorkflowMasonryCardMount.js';
import { WorkflowMasonrySectionHeaderView } from './WorkflowMasonrySectionHeaderView.js';
/** Retain the shared optional-node menu while reconciling native card membership. */
export class WorkflowMasonrySectionView {
    header;
    cells = new Map();
    #options;
    #headerView;
    #cards = new Map();
    /** Build stable section chrome using the existing optional-node toolbox presenter. */
    constructor(section, options) {
        this.#options = options;
        this.#headerView = new WorkflowMasonrySectionHeaderView(section, {
            document: options.content.ownerDocument,
            cardState: options.cardState,
            openCube: options.openCube,
        });
        this.header = this.#headerView.element;
        options.content.append(this.header);
    }
    /** Reconcile shared policy decisions without replacing the live menu or untouched controls. */
    prepare(section, renderer, sources) {
        this.#headerView.render(section);
        const visible = section.cards.filter((card) => card.visible);
        const ids = new Set(visible.map((card) => card.id));
        for (const [id, record] of this.#cards) {
            const next = visible.find((card) => card.id === id);
            if (!ids.has(id)) {
                this.cells.delete(id);
                if (!record.retiring) {
                    record.retiring = true;
                    record.cell.style.pointerEvents = 'none';
                    this.#options.motion.retire(record.cell, () => {
                        releaseCard(record);
                        this.#cards.delete(id);
                    });
                }
            }
            else if (next?.node !== record.node ||
                next.showActivationControl !== record.activation ||
                (next.prompt === true) !== record.prompt) {
                this.#options.motion.forget(record.cell);
                releaseCard(record);
                this.#cards.delete(id);
                this.cells.delete(id);
            }
        }
        const pending = [];
        for (const card of visible) {
            let record = this.#cards.get(card.id);
            if (!record) {
                const cell = this.header.ownerDocument.createElement('article');
                cell.className = 'sugarcubes-workflow-masonry__card';
                cell.style.position = 'absolute';
                cell.dataset.masonryCard = card.id;
                cell.dataset.masonrySection = section.id;
                this.#options.content.append(cell);
                this.cells.set(card.id, cell);
                record = {
                    cell,
                    mount: new WorkflowMasonryCardMount({
                        cell,
                        card,
                        renderer,
                        source: section.cube ? (sources.get(card.id) ?? null) : null,
                        getSource: () => (section.cube ? this.#options.getSource(card.id) : null),
                        onActivationChange: (node, enabled) => this.#options.cardState.setEnabled(section.target, String(node.id ?? ''), enabled),
                        onRemounted: this.#options.onRemounted,
                    }),
                    node: card.node,
                    activation: card.showActivationControl,
                    prompt: card.prompt === true,
                    mounted: false,
                    retiring: false,
                };
                this.#cards.set(card.id, record);
            }
            if (record.retiring) {
                record.retiring = false;
                record.cell.style.removeProperty('pointer-events');
                this.cells.set(card.id, record.cell);
                this.#options.motion.revive(record.cell);
            }
            if (record.mounted) {
                record.cell.dataset.cubeColumnSpan = String(card.columnSpan);
                record.mount.refresh(card);
            }
            else {
                const mount = record;
                pending.push({
                    requiresNativeMount: mount.mount.requiresNativeMount,
                    mount: () => {
                        if (this.#cards.get(card.id) !== mount || mount.mounted)
                            return;
                        mount.mount.mount();
                        mount.mounted = true;
                    },
                });
            }
        }
        return pending;
    }
    /** Release all native leases and section chrome when its workflow membership ends. */
    dispose() {
        for (const record of this.#cards.values()) {
            this.#options.motion.forget(record.cell);
            releaseCard(record);
        }
        this.#cards.clear();
        this.cells.clear();
        this.#headerView.dispose();
    }
}
/** Release one card without disturbing surviving native control instances. */
function releaseCard(record) {
    record.mount.dispose();
    record.cell.remove();
}
