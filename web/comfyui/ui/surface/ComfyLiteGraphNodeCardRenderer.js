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
/** Render exact Nodes 1.0 cards through Comfy's active LiteGraph canvas renderer. */
import { measureCubeFaceNodeBodyHeight } from './CubeFaceNodeMeasurement.js';
import { withCubeFaceNodePresentation } from './CubeFaceNodePresentationPolicy.js';
/** Own exact legacy canvas draws and the narrow Cube-face presentation boundary. */
export class ComfyLiteGraphNodeCardRenderer {
    #document;
    #canvasRenderer;
    #titleHeight;
    #devicePixelRatio;
    #widgetInteraction;
    #mounts = new Set();
    /** Bind the currently active Comfy LiteGraph renderer. */
    constructor(options) {
        this.#document = options.document;
        this.#canvasRenderer = options.canvasRenderer;
        this.#titleHeight = options.titleHeight;
        this.#devicePixelRatio = options.devicePixelRatio ?? (() => globalThis.devicePixelRatio || 1);
        this.#widgetInteraction = options.widgetInteraction ?? null;
    }
    /** Draw one exact internal graph node with Comfy's Nodes 1.0 renderer. */
    mount(target, node) {
        const canvas = this.#document.createElement('canvas');
        canvas.dataset.cubeFaceNative = 'nodes-1';
        canvas.className = 'sugarcubes-native-node-card sugarcubes-native-node-card--legacy';
        canvas.style.display = 'block';
        canvas.style.width = '100%';
        target.replaceChildren(canvas);
        let disposed = false;
        let interactionMount = null;
        let logicalWidth = 1;
        let logicalHeight = 1;
        const draw = () => {
            if (disposed)
                return;
            const width = Math.max(1, Number(node.size?.[0]) || 200);
            const height = measureCubeFaceNodeBodyHeight(node);
            const titleHeight = this.#titleHeight;
            logicalWidth = width;
            logicalHeight = height + titleHeight;
            const ratio = Math.max(1, this.#devicePixelRatio());
            canvas.width = Math.ceil(width * ratio);
            canvas.height = Math.ceil((height + titleHeight) * ratio);
            canvas.style.aspectRatio = `${width} / ${height + titleHeight}`;
            const context = canvas.getContext('2d');
            if (!context) {
                throw new Error('Canvas 2D context is unavailable for Nodes 1.0 Cube cards.');
            }
            context.setTransform(ratio, 0, 0, ratio, 0, 0);
            context.clearRect(0, 0, width, height + titleHeight);
            context.save();
            context.translate(0, titleHeight);
            drawNativeLiteGraphCubeCard(this.#canvasRenderer, node, context, {
                normalizeRendererScale: true,
                presentationHeight: height,
            });
            context.restore();
        };
        draw();
        interactionMount =
            this.#widgetInteraction?.attach(canvas, node, () => ({
                width: logicalWidth,
                height: logicalHeight,
            })) ?? null;
        const mount = {
            refresh: draw,
            unmount: () => {
                if (disposed)
                    return;
                disposed = true;
                interactionMount?.dispose();
                canvas.remove();
                this.#mounts.delete(mount);
            },
        };
        this.#mounts.add(mount);
        return mount;
    }
    /** Remove every legacy card owned by this renderer. */
    dispose() {
        for (const mount of [...this.#mounts])
            mount.unmount();
    }
}
/** Invoke Comfy's exact draw while masking only Cube-face-excluded chrome. */
export function drawNativeLiteGraphCubeCard(canvasRenderer, node, context, options = {}) {
    const presentationNode = node;
    const drawSlots = presentationNode.drawSlots;
    const drawCollapsedSlots = presentationNode.drawCollapsedSlots;
    const titleButtons = presentationNode.title_buttons;
    const previews = maskPreviewMedia(presentationNode);
    const originalSize = applyPresentationSize(presentationNode, options.presentationWidth, options.presentationHeight);
    const colors = applyPresentationTheme(presentationNode, options.theme);
    const activeGraphScale = canvasRenderer.ds?.scale;
    try {
        if (canvasRenderer.ds && options.normalizeRendererScale === true) {
            canvasRenderer.ds.scale = 1;
        }
        presentationNode.drawSlots = () => { };
        presentationNode.drawCollapsedSlots = () => { };
        presentationNode.title_buttons = [];
        withCubeFaceNodePresentation(presentationNode, () => {
            context.textBaseline = 'alphabetic';
            presentationNode.updateArea?.(context);
            canvasRenderer.drawNode(node, context);
        });
    }
    finally {
        if (canvasRenderer.ds &&
            activeGraphScale !== undefined &&
            options.normalizeRendererScale === true) {
            canvasRenderer.ds.scale = activeGraphScale;
        }
        presentationNode.drawSlots = drawSlots;
        presentationNode.drawCollapsedSlots = drawCollapsedSlots;
        presentationNode.title_buttons = titleButtons;
        restorePreviewMedia(presentationNode, previews);
        restorePresentationTheme(presentationNode, colors);
        restorePresentationSize(presentationNode, originalSize, context);
    }
}
/** Apply the parent Cube colors only while Comfy draws one projected card. */
function applyPresentationTheme(node, theme) {
    if (!theme)
        return [];
    const presentations = ['color', 'bgcolor'].map((field) => ({
        field,
        owned: Object.prototype.hasOwnProperty.call(node, field),
        value: node[field],
    }));
    node.color = theme.header;
    node.bgcolor = theme.body;
    return presentations;
}
/** Restore exact internal-node color values and property ownership after drawing. */
function restorePresentationTheme(node, presentations) {
    for (const presentation of presentations) {
        if (presentation.owned)
            Reflect.set(node, presentation.field, presentation.value);
        else
            Reflect.deleteProperty(node, presentation.field);
    }
}
/** Apply one temporary body size so Comfy lays out the exact face card. */
function applyPresentationSize(node, presentationWidth, presentationHeight) {
    const width = Number(presentationWidth);
    const height = Number(presentationHeight);
    if (!node.size)
        return null;
    const hasWidth = Number.isFinite(width) && width > 0;
    const hasHeight = Number.isFinite(height) && height > 0;
    if (!hasWidth && !hasHeight)
        return null;
    const originalSize = [Number(node.size[0]), Number(node.size[1])];
    if (hasWidth)
        node.size[0] = width;
    if (hasHeight)
        node.size[1] = height;
    return originalSize;
}
/** Restore internal graph geometry after the face-only native draw. */
function restorePresentationSize(node, originalSize, context) {
    if (originalSize !== null && node.size) {
        node.size[0] = originalSize[0];
        node.size[1] = originalSize[1];
    }
    node.updateArea?.(context);
}
/** Suppress only Comfy's standard local preview state during a Cube-face draw. */
function maskPreviewMedia(node) {
    const presentations = [];
    for (const field of ['imgs', 'animatedImages', 'imageIndex']) {
        presentations.push({
            field,
            owned: Object.prototype.hasOwnProperty.call(node, field),
            value: node[field],
        });
    }
    node.imgs = [];
    node.animatedImages = [];
    node.imageIndex = null;
    return presentations;
}
/** Restore preview fields without changing their original ownership semantics. */
function restorePreviewMedia(node, presentations) {
    for (const presentation of presentations) {
        if (!presentation.owned) {
            Reflect.deleteProperty(node, presentation.field);
            continue;
        }
        node[presentation.field] = presentation.value;
    }
}
