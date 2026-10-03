import { NativeNodeCardHost } from '../NativeNodeCardHost.js';
import { NativeWidgetElementLease } from './NativeWidgetElementLease.js';
import { NativeForwardedWidgetLease } from './NativeForwardedWidgetLease.js';
import { resolveMasonryWidgetSources } from './WorkflowMasonryWidgetSources.js';
import { WorkflowMasonrySectionHeaderView } from './WorkflowMasonrySectionHeaderView.js';
import { MasonryNodeIconHost } from './MasonryNodeIconHost.js';
/** Retain the shared optional-node menu while reconciling native card membership. */
export class WorkflowMasonrySectionView {
    header;
    cells = new Map();
    #options;
    #headerView;
    #cards = new Map();
    #cubeWidgets = null;
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
    prepare(section, renderer) {
        this.#headerView.render(section);
        if (!this.#cubeWidgets && section.cube)
            this.#cubeWidgets = new NativeWidgetElementLease([section.cube]);
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
            else if (next?.node !== record.node || next.showActivationControl !== record.activation) {
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
                this.#options.content.append(cell);
                this.cells.set(card.id, cell);
                record = {
                    cell,
                    host: new NativeNodeCardHost(renderer),
                    icon: new MasonryNodeIconHost(cell, card.node),
                    widgets: new NativeWidgetElementLease([card.node]),
                    forwarded: null,
                    node: card.node,
                    activation: card.showActivationControl,
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
                record.host.refresh([card]);
                record.icon.refresh();
            }
            else {
                const mount = record;
                pending.push(() => {
                    if (this.#cards.get(card.id) !== mount || mount.mounted)
                        return;
                    const widgetSources = section.cube
                        ? resolveMasonryWidgetSources(section.cube, card.node)
                        : [];
                    mount.forwarded = new NativeForwardedWidgetLease(mount.cell, card.node, widgetSources);
                    mount.host.mountCard(mount.cell, card, (node, enabled) => this.#options.cardState.setEnabled(section.target, String(node.id ?? ''), enabled), { presentationGeometry: true, preserveNativeBody: true, widgetSources });
                    mount.mounted = true;
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
        this.#cubeWidgets?.restore();
        this.#cubeWidgets = null;
        this.#headerView.dispose();
    }
}
/** Release one card without disturbing surviving native control instances. */
function releaseCard(record) {
    record.icon.dispose();
    record.forwarded?.dispose();
    record.host.dispose();
    record.widgets.restore();
    record.cell.remove();
}
