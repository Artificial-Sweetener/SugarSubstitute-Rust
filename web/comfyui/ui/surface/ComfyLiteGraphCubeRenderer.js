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
/** Draw custom Cube faces from one real Nodes 1.0 node lifecycle. */
import { drawCubeCanvasActivationControl } from './CubeCanvasActivationControl.js';
import { drawNativeLiteGraphCubeCard } from './ComfyLiteGraphNodeCardRenderer.js';
import { computeCubeCanvasCardMenuLayout } from './CubeCanvasCardMenuLayout.js';
import { CubeCanvasChromeRenderer, } from './CubeCanvasChromeRenderer.js';
import { drawComfyPrimeIcon } from './ComfyPrimeIcons.js';
import { CUBE_PREVIEW_SECTION_GAP, CUBE_PREVIEW_SECTION_INSET, CUBE_PREVIEW_TITLE_LINE_HEIGHT, dividePreviewIntoHorizontalSegments, resolveCubeCanvasPreviewSections, } from './CubePreviewSections.js';
/** Own only visual composition for legacy canvas Cube surfaces. */
export class ComfyLiteGraphCubeRenderer {
    #host;
    #previewImages;
    #chrome;
    /** Bind Comfy's exact active node and boundary render surfaces. */
    constructor(host, previewImages, icons) {
        this.#host = host;
        this.#previewImages = previewImages;
        this.#chrome = new CubeCanvasChromeRenderer(icons);
    }
    /** Draw one ordered set of graph-space Cube surfaces. */
    draw(context, items) {
        for (const item of items)
            this.#drawCube(context, item);
    }
    /** Draw one frame, native cards, preview rail, and graph-owned ports. */
    #drawCube(context, item) {
        const { layout } = item;
        context.save();
        context.beginPath();
        context.roundRect(layout.frame.x + 1, layout.frame.y + 1, Math.max(1, layout.frame.width - 2), Math.max(1, layout.frame.height - 2), 13);
        context.fillStyle = '#0d1117';
        context.fill();
        context.clip();
        this.#chrome.draw(context, item);
        for (const card of layout.cards)
            this.#drawNativeCard(context, card);
        if (layout.preview)
            this.#drawPreview(context, layout.preview, item);
        if (item.cardMenuOpen)
            this.#drawCardMenu(context, layout);
        context.restore();
    }
    /** Invoke Comfy's exact Nodes 1.0 draw against the real internal node. */
    #drawNativeCard(context, card) {
        const { node, rect: target, bodyHeight } = card;
        const titleHeight = Math.max(1, target.height - bodyHeight);
        context.save();
        context.beginPath();
        context.rect(target.x, target.y, target.width, target.height);
        context.clip();
        context.translate(target.x, target.y + titleHeight);
        drawNativeLiteGraphCubeCard(this.#host, node, context, {
            presentationWidth: target.width,
            presentationHeight: bodyHeight,
        });
        context.restore();
        if (card.activationAction) {
            drawCubeCanvasActivationControl(context, card.activationAction, card.enabled);
        }
    }
    /** Draw Cube-owned optional-card reveal choices above the native masonry. */
    #drawCardMenu(context, layout) {
        const menu = computeCubeCanvasCardMenuLayout(layout);
        context.fillStyle = '#171b20';
        context.strokeStyle = 'rgba(220, 225, 235, 0.3)';
        context.lineWidth = 1;
        context.beginPath();
        context.roundRect(menu.rect.x, menu.rect.y, menu.rect.width, menu.rect.height, 6);
        context.fill();
        context.stroke();
        context.font = '12px sans-serif';
        context.textBaseline = 'middle';
        for (const item of menu.items) {
            context.fillStyle = item.entry.revealed ? '#f0f2f5' : '#7f8792';
            drawComfyPrimeIcon(context, item.entry.revealed ? 'circle-fill' : 'circle', item.rect.x + 4, item.rect.y + item.rect.height / 2);
            context.fillText(item.entry.label, item.rect.x + 24, item.rect.y + item.rect.height / 2, Math.max(1, item.rect.width - 28));
        }
    }
    /** Draw every output in equal horizontal sections using the dedicated cache. */
    #drawPreview(context, preview, item) {
        context.strokeStyle = 'rgba(220, 225, 235, 0.22)';
        context.lineWidth = 1;
        context.beginPath();
        context.moveTo(preview.x - 6, preview.y);
        context.lineTo(preview.x - 6, preview.y + preview.height);
        context.stroke();
        const outputSections = resolveCubeCanvasPreviewSections(item.preview);
        if (outputSections.length === 0) {
            context.font = '12px sans-serif';
            context.textBaseline = 'top';
            context.fillStyle = '#aeb5c0';
            context.fillText('No preview available', preview.x + 6, preview.y + 6);
            return;
        }
        const sections = dividePreviewIntoHorizontalSegments({
            x: preview.x + CUBE_PREVIEW_SECTION_INSET,
            y: preview.y + CUBE_PREVIEW_SECTION_INSET,
            width: Math.max(1, preview.width - CUBE_PREVIEW_SECTION_INSET * 2),
            height: Math.max(1, preview.height - CUBE_PREVIEW_SECTION_INSET * 2),
        }, outputSections.length, CUBE_PREVIEW_SECTION_GAP);
        for (const [index, output] of outputSections.entries()) {
            const section = sections[index];
            if (!section)
                continue;
            context.font = '14px sans-serif';
            context.textBaseline = 'top';
            context.fillStyle = '#f0f2f5';
            context.fillText(output.canonicalName, section.x, section.y, Math.max(1, section.width));
            const media = output.item;
            if (!media) {
                context.font = '12px sans-serif';
                context.fillStyle = '#aeb5c0';
                context.fillText('No preview available', section.x, section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT);
                continue;
            }
            const image = this.#previewImages.get(media.url, media.sourceLocator);
            if (!image) {
                context.font = '12px sans-serif';
                context.fillStyle = '#aeb5c0';
                context.fillText('Loading output…', section.x, section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT);
                continue;
            }
            const target = containImage(image.naturalWidth, image.naturalHeight, section.x, section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT, section.width, Math.max(1, section.height - CUBE_PREVIEW_TITLE_LINE_HEIGHT));
            context.drawImage(image, target.x, target.y, target.width, target.height);
        }
    }
}
/** Fit one image inside the available rail while preserving its aspect ratio. */
function containImage(sourceWidth, sourceHeight, x, y, width, height) {
    const scale = Math.min(width / sourceWidth, height / sourceHeight);
    const targetWidth = Math.max(1, sourceWidth * scale);
    const targetHeight = Math.max(1, sourceHeight * scale);
    return {
        x: x + (width - targetWidth) / 2,
        y: y + (height - targetHeight) / 2,
        width: targetWidth,
        height: targetHeight,
    };
}
