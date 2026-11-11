//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Adapt real Cube-graph nodes to Comfy's host-owned native card renderer. */
import { CubeFaceActivationControl } from './CubeFaceActivationControl.js';
import { cubeFaceNodeHasVisibleWidgets } from './CubeFaceNodePresentationPolicy.js';
import { NativeWidgetElementLease } from './NativeWidgetElementLease.js';
import { NativeForwardedWidgetLease } from './NativeForwardedWidgetLease.js';
import { NodeCardIconHost } from './NodeCardIconHost.js';
import { registerNativeNodeCard } from './NativeNodeCardBoundary.js';
/** Own native child-card mounts for one Cube surface view. */
export class NativeNodeCardHost {
    #renderer;
    #mounts = [];
    #activation = new Map();
    #leases = [];
    #ownedCells = [];
    /** Create a host that delegates all node rendering to Comfy. */
    constructor(renderer) {
        this.#renderer = renderer;
    }
    /** Mount the exact internal graph nodes into dedicated masonry cells. */
    mount(container, cards, onActivationChange) {
        this.dispose();
        container.replaceChildren();
        const cells = [];
        for (const card of cards) {
            const cell = container.ownerDocument.createElement('div');
            container.appendChild(cell);
            this.#ownedCells.push(cell);
            this.mountCard(cell, card, onActivationChange, card.prompt === true ? { allowCollapse: false } : {});
            cells.push(cell);
        }
        return cells;
    }
    /** Mount shared presentation cards with native controls and read-only node titles. */
    mountCard(cell, card, onActivationChange, options = {}) {
        const { node } = card;
        cell.classList.add('sugarcubes-cube-face__node-card');
        this.#leases.push(registerNativeNodeCard(cell, node, card.cubeId ?? null));
        cell.dataset.cubeNodeId = String(node.id ?? '');
        cell.dataset.cubeColumnSpan = String(card.columnSpan ?? 1);
        if (node.graph?.id !== undefined)
            cell.dataset.cubeNodeLocator = `${String(node.graph.id)}:${String(node.id ?? '')}`;
        const target = cell.ownerDocument.createElement('div');
        target.className = 'sugarcubes-cube-face__native-card-mount';
        cell.append(target);
        const activation = card.showActivationControl
            ? new CubeFaceActivationControl(cell.ownerDocument, card, onActivationChange)
            : null;
        if (activation)
            this.#activation.set(node, activation);
        const widgetSources = card.widgetSources ?? [];
        const widgets = new NativeWidgetElementLease([
            node,
            ...widgetSources.map((source) => source.node),
        ]);
        const forwarded = new NativeForwardedWidgetLease(cell, node, widgetSources);
        const icon = new NodeCardIconHost(cell, node);
        this.#leases.push(() => {
            icon.dispose();
            forwarded.dispose();
            widgets.restore();
        });
        this.#mounts.push(this.#renderer.mount(target, node, {
            presentationGeometry: true,
            preserveNativeBody: cubeFaceNodeHasVisibleWidgets(node) || widgetSources.length > 0,
            widgetSources,
            ...options,
            allowRename: false,
            ...(activation ? { headerAccessory: activation.root } : {}),
        }));
    }
    /** Refresh render data after an internal node changes structure. */
    refresh(cards = []) {
        for (const card of cards) {
            const control = this.#activation.get(card.node);
            if (control)
                control.update(card);
        }
        for (const mount of this.#mounts)
            mount.refresh();
    }
    /** Unmount this view's cards and remove the cells it created, keeping caller-owned cells. */
    dispose() {
        for (const control of this.#activation.values())
            control.dispose();
        for (const mount of this.#mounts.splice(0))
            mount.unmount();
        for (const release of this.#leases.splice(0))
            release();
        for (const cell of this.#ownedCells.splice(0))
            cell.remove();
        this.#activation.clear();
    }
}
