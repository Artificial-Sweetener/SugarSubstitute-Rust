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
/** Render Nodes 1.0 Cube header chrome with Comfy-owned icon primitives. */
import { resolveCubeIdentityPresentation } from '../cube/CubeIdentityPresentation.js';
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { drawFallbackInitialsCanvas } from '../core/CubeFallbackIconRenderer.js';
import { resolveCubeFaceTitlebarActions, } from './CubeFaceChromeActions.js';
import { drawComfyPrimeIcon } from './ComfyPrimeIcons.js';
/** Own Cube header composition without owning cards, previews, or interaction. */
export class CubeCanvasChromeRenderer {
    #icons;
    /** Bind Cube-definition icon loading to the header renderer. */
    constructor(icons) {
        this.#icons = icons;
    }
    /** Draw one Cube header and every currently available action. */
    draw(context, item) {
        const { node, layout } = item;
        const identity = resolveCubeIdentityPresentation({
            metadata: requireCubeIdentity(node),
            instanceTitle: node.title?.trim() || node.subgraph.name,
            fallbackDefinitionTitle: node.subgraph.name,
        });
        context.fillStyle = '#12161b';
        context.fillRect(layout.header.x, layout.header.y, layout.header.width, layout.header.height);
        context.font = '600 16px sans-serif';
        context.textBaseline = 'middle';
        const iconSize = 24;
        const iconX = layout.header.x + 12;
        const iconY = layout.header.y + (layout.header.height - iconSize) / 2;
        this.#drawDefinitionIcon(context, identity.icon, iconX, iconY, iconSize);
        context.fillStyle = '#f0f2f5';
        context.fillText(identity.instanceTitle, iconX + iconSize + 7, layout.header.y + layout.header.height / 2, Math.max(1, layout.header.width / 2 - iconSize - 34));
        context.save();
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        const definitionCenterX = layout.header.x + layout.header.width / 2;
        const definitionMaxWidth = Math.max(1, layout.editAction.x - definitionCenterX - 8);
        context.font = '12px sans-serif';
        context.fillStyle = '#d7dbe2';
        context.fillText(identity.definitionLine, definitionCenterX, layout.header.y + layout.header.height / 2 - 6, definitionMaxWidth);
        context.font = '10px sans-serif';
        context.fillStyle = '#9aa2ad';
        context.fillText(identity.sourceLine, definitionCenterX, layout.header.y + layout.header.height / 2 + 7, definitionMaxWidth);
        context.restore();
        drawNativeEditorButton(context, layout.editAction, item.editorButton);
        if (layout.cardMenuEntries.length > 0) {
            drawPrimeIconAction(context, layout.cardMenuAction, 'eye');
        }
        for (const action of resolveCubeFaceTitlebarActions(requireCubeIdentity(node), item.chromeActions)) {
            const target = layout.chromeActions[action.key];
            if (target)
                drawPrimeIconAction(context, target, action.icon);
        }
    }
    /** Draw one definition asset or its exact initials fallback. */
    #drawDefinitionIcon(context, model, x, y, size) {
        const entry = this.#icons.getImage(model);
        if (model.kind === 'asset' && entry.status === 'ready' && entry.image) {
            const width = Math.max(1, entry.image.naturalWidth);
            const height = Math.max(1, entry.image.naturalHeight);
            const scale = Math.min(size / width, size / height);
            const targetWidth = width * scale;
            const targetHeight = height * scale;
            context.drawImage(entry.image, x + (size - targetWidth) / 2, y + (size - targetHeight) / 2, targetWidth, targetHeight);
            return;
        }
        drawFallbackInitialsCanvas(context, model, x, y, size);
    }
}
/** Reuse the real SubgraphNode title button, preserving Comfy's exact icon drawing. */
function drawNativeEditorButton(context, target, button) {
    if (!button?.visible) {
        context.fillStyle = '#f0f2f5';
        drawComfyPrimeIcon(context, 'window-maximize', target.x + target.width / 2, target.y + target.height / 2);
        return;
    }
    const width = button.getWidth(context);
    const x = target.x + (target.width - width) / 2 - button.xOffset;
    const y = target.y + (target.height - button.height) / 2 - button.yOffset;
    context.save();
    context.fillStyle = '#f0f2f5';
    button.draw(context, x, y);
    context.restore();
}
/** Draw one icon-only Cube action using Comfy's PrimeIcons font. */
function drawPrimeIconAction(context, target, icon) {
    context.fillStyle = '#f0f2f5';
    drawComfyPrimeIcon(context, icon, target.x + target.width / 2, target.y + target.height / 2);
}
