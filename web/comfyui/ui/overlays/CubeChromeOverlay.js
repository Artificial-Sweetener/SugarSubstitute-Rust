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
/**
 * Coordinate Cube chrome lifecycle, rendering, and interaction owners.
 */
import { CubeChromeGroupTitleHook } from './CubeChromeGroupTitleHook.js';
import { CubeChromeInteraction } from './CubeChromeInteraction.js';
import { CubeChromePainter } from './CubeChromePainter.js';
import { CubeChromeRenderer } from './CubeChromeRenderer.js';
/** Preserve the host-facing Cube chrome API across focused owners. */
export class CubeChromeOverlay {
    actions;
    painter;
    renderer;
    interaction;
    groupTitleHook;
    adapter;
    constructor({ adapter = null, actions = {}, resolveSource = null } = {}) {
        this.adapter = adapter;
        this.actions = actions || {};
        this.painter = new CubeChromePainter(() => this.requestRedraw(), (ctx, x, y, size, color) => this.drawCubeIcon(ctx, x, y, size, color));
        this.renderer = new CubeChromeRenderer({
            getActions: () => this.actions,
            getHoverState: () => this.interaction.getHoverState(),
            resolveSource: typeof resolveSource === 'function' ? resolveSource : null,
            requestRedraw: () => this.requestRedraw(),
            painter: this.painter,
        });
        this.interaction = new CubeChromeInteraction({
            getActions: () => this.actions,
            getHitRegions: () => this.renderer.hitRegions,
            getBadgeRegions: () => this.renderer.badgeRegions,
            requestRedraw: () => this.requestRedraw(),
        });
        this.groupTitleHook = new CubeChromeGroupTitleHook(adapter, this.painter);
    }
    get lastTransform() {
        return this.renderer.lastTransform;
    }
    buildMenuOptions(options) {
        return this.interaction.buildMenuOptions(options);
    }
    setup() {
        this.groupTitleHook.installGroupTitleIconRenderer();
    }
    dispose() {
        this.renderer.clear();
        this.interaction.clear();
        this.groupTitleHook.releaseGroupTitleIconRenderer();
    }
    setActions(actions) {
        this.actions = actions || {};
    }
    getDebugState() {
        const hover = this.interaction.getHoverState();
        return {
            actions: this.actions,
            hitRegions: this.renderer.hitRegions,
            badgeRegions: this.renderer.badgeRegions,
            hoveredKey: hover.hoveredKey,
            hoveredBadgeInstance: hover.hoveredBadgeInstance,
            hoveredBadgeKey: hover.hoveredBadgeKey,
        };
    }
    installGroupTitleIconRenderer() {
        this.groupTitleHook.installGroupTitleIconRenderer();
    }
    releaseGroupTitleIconRenderer() {
        this.groupTitleHook.releaseGroupTitleIconRenderer();
    }
    shouldDrawGroupTitleIcon(group) {
        return this.groupTitleHook.shouldDrawGroupTitleIcon(group);
    }
    drawManagedGroupWithTitleIcon(group, originalDraw, args) {
        return this.groupTitleHook.drawManagedGroupWithTitleIcon(group, originalDraw, args);
    }
    drawGroupTitleIcon(ctx, group, canvasInstance) {
        this.painter.drawGroupTitleIcon(ctx, group, canvasInstance);
    }
    render(ctx, canvasInstance) {
        this.renderer.render(ctx, canvasInstance);
    }
    renderHeader(ctx, bounds, metadata, group = {}) {
        this.renderer.renderHeader(ctx, bounds, metadata, group);
    }
    drawPill(ctx, x, y, w, h, style, options = {}) {
        this.painter.drawPill(ctx, x, y, w, h, style, options);
    }
    drawIcon(ctx, icon, x, y, size, color, options = {}) {
        this.painter.drawIcon(ctx, icon, x, y, size, color, options);
    }
    drawPenIcon(ctx, x, y, size, color) {
        this.painter.drawPenIcon(ctx, x, y, size, color);
    }
    drawCubeIcon(ctx, x, y, size, color) {
        this.painter.drawCubeIcon(ctx, x, y, size, color);
    }
    drawDefinitionIcon(ctx, model, x, y, size, options = {}) {
        this.painter.drawDefinitionIcon(ctx, model, x, y, size, options);
    }
    drawContainedImage(ctx, image, x, y, size) {
        this.painter.drawContainedImage(ctx, image, x, y, size);
    }
    handleMouseDown(event, canvasInstance) {
        return this.interaction.handleMouseDown(event, canvasInstance);
    }
    handlePointerMove(event, canvasInstance) {
        return this.interaction.handlePointerMove(event, canvasInstance);
    }
    drawTooltip(ctx, centerX, topY, text, fontSize) {
        this.painter.drawTooltip(ctx, centerX, topY, text, fontSize);
    }
    isAnimating(state, now) {
        return this.renderer.isAnimating(state, now);
    }
    ensureInstanceState(instanceId) {
        return this.renderer.ensureInstanceState(instanceId);
    }
    isHovered(instanceId, key) {
        return this.renderer.isHovered(instanceId, key);
    }
    requestRedraw() {
        this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
    }
    convertEventToCanvasPoint(event, canvasInstance) {
        return this.interaction.convertEventToCanvasPoint(event, canvasInstance);
    }
    convertCanvasPoint(canvasInstance, point) {
        return this.interaction.convertCanvasPoint(canvasInstance, point);
    }
}
