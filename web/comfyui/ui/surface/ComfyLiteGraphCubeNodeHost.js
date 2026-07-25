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
/** Install custom Cube faces into real Nodes 1.0 node draw lifecycles. */
import { requireCubeIdentity, requireCubeSurface, } from '../cube/node/ComfyCubeNodeFactory.js';
import { ComfyLiteGraphCubeNodeInteraction, } from './ComfyLiteGraphCubeNodeInteraction.js';
import { ComfyLiteGraphCubeDomWidgetHost } from './ComfyLiteGraphCubeDomWidgetHost.js';
import { ComfyLiteGraphCubeRenderer, } from './ComfyLiteGraphCubeRenderer.js';
import { ComfyLiteGraphCubeBoundaryHost } from './ComfyLiteGraphCubeBoundaryHost.js';
import { CubeIconResolver } from '../core/CubeIconResolver.js';
import { ComfyLiteGraphWidgetInteraction } from './ComfyLiteGraphWidgetInteraction.js';
import { computeCubeCanvasLayout } from './CubeCanvasLayout.js';
import { CubeCanvasPreviewImageCache, } from './CubeCanvasPreviewImageCache.js';
import { enforceCubeNodeMinimumHeight } from './CubeNodeMinimumHeightAdapter.js';
import { ComfyGraphPreviewImageSource } from './ComfyGraphPreviewImageSource.js';
import { resolveCubeFaceTitlebarActions, } from './CubeFaceChromeActions.js';
import { setCubeFaceCardRevealed, setCubeFaceNodeEnabled } from './CubeFaceCardStateController.js';
import { parseCubeSurfaceState, serializeCubeSurfaceState, } from './CubeSurfaceState.js';
/** Own the narrow Nodes 1.0 draw and interaction seam for native Cube nodes. */
export class ComfyLiteGraphCubeNodeHost {
    #canvas;
    #rootGraph;
    #nodes;
    #history;
    #titleHeight;
    #previewCatalog;
    #chromeActions;
    #renderer;
    #boundaryHost = new ComfyLiteGraphCubeBoundaryHost();
    #interaction;
    #domWidgets;
    #hooks = new Map();
    #enabled = false;
    #items = [];
    #openCardMenu = null;
    /** Bind native draw hooks and focused face interactions. */
    constructor(options) {
        this.#canvas = options.canvas;
        this.#rootGraph = options.rootGraph;
        this.#nodes = options.nodes;
        this.#history = options.history;
        this.#titleHeight = Math.max(1, options.titleHeight);
        this.#previewCatalog = options.previewCatalog ?? null;
        this.#chromeActions = options.chromeActions ?? null;
        const graphImages = new ComfyGraphPreviewImageSource(options.document, options.rootGraph, options.logger ?? null);
        const previewImages = options.previewImages ??
            new CubeCanvasPreviewImageCache({
                createImage: () => options.document.createElement('img'),
                findLoadedImage: (url, sourceLocator) => graphImages.find(url, sourceLocator),
                invalidate: () => this.#refresh(),
                ...(options.logger ? { logger: options.logger } : {}),
            });
        this.#renderer = new ComfyLiteGraphCubeRenderer(options.canvas, previewImages, new CubeIconResolver({
            imageFactory: () => options.document.createElement('img'),
            onImageLoad: () => this.#refresh(),
        }));
        this.#domWidgets = new ComfyLiteGraphCubeDomWidgetHost({
            document: options.document,
            canvas: options.canvas,
            ...(options.logger ? { logger: options.logger } : {}),
        });
        const widgetInteraction = new ComfyLiteGraphWidgetInteraction({
            graphMouse: options.canvas.graph_mouse,
            processWidgetClick: (event, node, widget, pointer) => options.canvas.processWidgetClick(event, node, widget, pointer),
            titleHeight: this.#titleHeight,
        });
        this.#interaction = new ComfyLiteGraphCubeNodeInteraction({
            canvas: options.canvas,
            history: options.history,
            widgetInteraction,
            getItems: () => this.#items,
            chromeActions: this.#chromeActions,
            onEdit: options.openEditor,
            onCardMenuToggle: (node) => {
                this.#openCardMenu = this.#openCardMenu === node ? null : node;
                this.sync();
            },
            onCardRevealChange: (node, internalNode, revealed) => this.#updateSurface(node, (state) => setCubeFaceCardRevealed(state, internalNode, revealed)),
            onCardActivationChange: (node, internalNode, enabled) => this.#updateSurface(node, (state) => setCubeFaceNodeEnabled(state, internalNode, enabled)),
        });
    }
    /** Enable the custom draw face only while Nodes 1.0 displays the root graph. */
    setEnabled(enabled) {
        this.#enabled = enabled;
        this.sync();
    }
    /** Reconcile draw hooks and current face geometry with graph navigation. */
    sync() {
        const active = this.#enabled && this.#canvas.graph === this.#rootGraph;
        const nodes = active ? new Set(this.#nodes.list()) : new Set();
        for (const mounted of [...this.#hooks.keys()]) {
            if (!nodes.has(mounted))
                this.#unmount(mounted);
        }
        if (!active) {
            this.#items = [];
            this.#domWidgets.sync([]);
            this.#refresh();
            return;
        }
        for (const node of nodes) {
            if (!this.#hooks.has(node))
                this.#mount(node);
        }
        this.#items = [...nodes].map((node) => this.#renderItem(node));
        this.#domWidgets.sync(this.#items);
        this.#refresh();
    }
    /** Restore every native node method and release focused pointer routing. */
    dispose() {
        this.#interaction.dispose();
        this.#domWidgets.dispose();
        for (const node of [...this.#hooks.keys()])
            this.#unmount(node);
        this.#boundaryHost.dispose();
        this.#items = [];
        this.#enabled = false;
        this.#refresh();
    }
    /** Replace only generic subgraph face drawing on one real node. */
    #mount(node) {
        const drawNode = node;
        const hooks = {
            foreground: drawNode.onDrawForeground,
            drawWidgets: drawNode.drawWidgets,
            drawSlots: drawNode.drawSlots,
            titleButtons: drawNode.title_buttons,
        };
        drawNode.onDrawForeground = (context) => {
            if (!this.#enabled || this.#canvas.graph !== this.#rootGraph) {
                hooks.foreground?.call(drawNode, context, this.#canvas, this.#canvas.canvas);
                return;
            }
            const item = this.#renderItem(node);
            this.#replaceItem(item);
            context.save();
            context.translate(-Number(node.pos[0]), -Number(node.pos[1]));
            this.#renderer.draw(context, [item]);
            context.restore();
            this.#domWidgets.sync(this.#items);
        };
        drawNode.drawWidgets = () => undefined;
        drawNode.drawSlots = (context, drawOptions) => this.#boundaryHost.drawNativeSlotsWithoutOutputLabels(node, context, () => hooks.drawSlots?.call(drawNode, context, drawOptions));
        drawNode.title_buttons = [];
        this.#hooks.set(node, hooks);
    }
    /** Restore one node's original generic subgraph presentation hooks. */
    #unmount(node) {
        const hooks = this.#hooks.get(node);
        if (!hooks)
            return;
        const drawNode = node;
        restoreOptional(drawNode, 'onDrawForeground', hooks.foreground);
        restoreOptional(drawNode, 'drawWidgets', hooks.drawWidgets);
        restoreOptional(drawNode, 'drawSlots', hooks.drawSlots);
        restoreOptional(drawNode, 'title_buttons', hooks.titleButtons);
        this.#boundaryHost.release(node);
        this.#hooks.delete(node);
    }
    /** Build one current face layout from the graph-owned node and persisted state. */
    #renderItem(node) {
        const state = parseCubeSurfaceState(requireCubeSurface(node));
        const titlebarActionKeys = resolveCubeFaceTitlebarActions(requireCubeIdentity(node), this.#chromeActions).map((action) => action.key);
        let layout = computeCubeCanvasLayout(node, state, this.#titleHeight, titlebarActionKeys);
        if (enforceCubeNodeMinimumHeight(node, layout.minimumSize[1])) {
            layout = computeCubeCanvasLayout(node, state, this.#titleHeight, titlebarActionKeys);
            this.#history.setDirtyCanvas?.(true, true);
            this.#refresh();
        }
        this.#boundaryHost.sync(node, layout.outputs, layout.preview ? layout.preview.x - Number(node.pos[0]) : undefined);
        return {
            node,
            layout,
            cardMenuOpen: this.#openCardMenu === node,
            preview: this.#previewCatalog?.snapshot(node) ?? null,
            chromeActions: this.#chromeActions,
            editorButton: findNativeEditorButton(this.#hooks.get(node)?.titleButtons),
        };
    }
    /** Keep DOM-widget reconciliation on the same freshly rendered geometry. */
    #replaceItem(item) {
        const index = this.#items.findIndex((candidate) => candidate.node === item.node);
        if (index < 0) {
            this.#items.push(item);
            return;
        }
        this.#items[index] = item;
    }
    /** Persist one focused Cube-face state transition through native history. */
    #updateSurface(node, update) {
        const surface = requireCubeSurface(node);
        const state = parseCubeSurfaceState(surface);
        this.#history.beforeChange?.();
        update(state);
        replaceRecord(surface, serializeCubeSurfaceState(state));
        this.#nodes.changed(node);
        this.#history.setDirtyCanvas?.(true, true);
        this.#history.afterChange?.();
        this.sync();
    }
    /** Request one native canvas repaint. */
    #refresh() {
        this.#canvas.setDirty?.(true, true);
    }
}
/** Replace persisted face state while retaining node-property ownership. */
function replaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
/** Restore an optional host property without assigning explicit undefined. */
function restoreOptional(target, key, value) {
    if (value === undefined) {
        Reflect.deleteProperty(target, key);
        return;
    }
    target[key] = value;
}
/** Select Comfy's own SubgraphNode edit control from the preserved native buttons. */
function findNativeEditorButton(buttons) {
    for (const button of buttons ?? []) {
        if (!isNativeEditorButton(button) || button.name !== 'enter_subgraph')
            continue;
        return button;
    }
    return null;
}
/** Narrow the dynamic LiteGraph button surface required for exact native drawing. */
function isNativeEditorButton(value) {
    if (!value || typeof value !== 'object')
        return false;
    const button = value;
    return (typeof button.getWidth === 'function' &&
        typeof button.draw === 'function' &&
        typeof button.height === 'number' &&
        typeof button.visible === 'boolean' &&
        typeof button.xOffset === 'number' &&
        typeof button.yOffset === 'number');
}
