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
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/ProximityOverlay.js`.
 */
import { isRecord } from '../types/common.js';
import { ComfyGraphGeometry } from './proximity/ComfyGraphGeometry.js';
import { ProximityMatcher, } from './proximity/ProximityMatcher.js';
import { ProximityPromptPatcher } from './proximity/ProximityPromptPatcher.js';
const PROXIMITY_STORAGE_KEY = 'SugarCubes.Proximity.Settings';
const DEFAULT_PROXIMITY_SETTINGS = Object.freeze({
    enabled: true,
    radius: 160,
    strict: true,
    showOverlay: true,
});
function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
}
/**
 * Coordinate proximity overlay behavior for the SugarCubes UI.
 */
export class ProximityOverlay {
    adapter;
    events;
    scheduler;
    storage;
    logger;
    graphGeometry;
    matcher;
    promptPatcher = new ProximityPromptPatcher();
    settings;
    overlayMatches;
    promptMatches;
    previewScheduled;
    overlayActive;
    lastReportedMatchSignature = '';
    lastReportedRenderSignature = '';
    constructor({ adapter = null, events = null, scheduler = null, storage = null, } = {}) {
        this.adapter = adapter;
        this.events = events;
        this.scheduler = scheduler;
        this.storage = storage;
        this.logger = adapter?.getConsole?.() || null;
        this.graphGeometry = new ComfyGraphGeometry(this.logger ?? console);
        this.matcher = new ProximityMatcher({ discover: () => ({ outputs: [], inputs: [] }) }, () => this.adapter?.getLiteGraph?.() ?? null, this.logger ?? console);
        this.settings = this.loadSettings();
        this.overlayMatches = [];
        this.promptMatches = [];
        this.previewScheduled = false;
        this.overlayActive = this.isOverlayEnabled();
    }
    /** Replace endpoint discovery when the current graph-scoped Cube runtime changes. */
    setEndpointSource(source) {
        this.matcher = new ProximityMatcher(source, () => this.adapter?.getLiteGraph?.() ?? null, this.logger ?? console);
        this.refreshOverlayState({
            recompute: true,
            graph: this.adapter?.getApp?.()?.graph ?? null,
        });
    }
    loadSettings() {
        try {
            const stored = this.storage?.readJson?.(PROXIMITY_STORAGE_KEY);
            if (!stored || typeof stored !== 'object') {
                return { ...DEFAULT_PROXIMITY_SETTINGS };
            }
            return {
                enabled: typeof stored.enabled === 'boolean' ? stored.enabled : DEFAULT_PROXIMITY_SETTINGS.enabled,
                radius: typeof stored.radius === 'number' ? stored.radius : DEFAULT_PROXIMITY_SETTINGS.radius,
                strict: typeof stored.strict === 'boolean' ? stored.strict : DEFAULT_PROXIMITY_SETTINGS.strict,
                showOverlay: typeof stored.showOverlay === 'boolean'
                    ? stored.showOverlay
                    : DEFAULT_PROXIMITY_SETTINGS.showOverlay,
            };
        }
        catch (error) {
            this.logger?.warn?.('SugarCubes: failed to load proximity settings', error);
            return { ...DEFAULT_PROXIMITY_SETTINGS };
        }
    }
    persistSettings() {
        try {
            this.storage?.writeJson?.(PROXIMITY_STORAGE_KEY, this.settings);
        }
        catch (error) {
            this.logger?.warn?.('SugarCubes: failed to persist proximity settings', error);
        }
    }
    isOverlayEnabled() {
        return Boolean(this.settings.enabled && this.settings.showOverlay);
    }
    setEnabled(enabled) {
        this.settings.enabled = Boolean(enabled);
        if (!this.settings.enabled) {
            this.promptMatches = [];
        }
        this.persistSettings();
        this.refreshOverlayState({ recompute: true });
        return this.settings.enabled;
    }
    toggle() {
        return this.setEnabled(!this.settings.enabled);
    }
    applyProximityToPrompt(data) {
        if (!this.settings.enabled) {
            this.promptMatches = [];
            this.updateOverlay([]);
            return data;
        }
        if (!isRecord(data) || !isRecord(data.output)) {
            this.promptMatches = [];
            this.updateOverlay([]);
            return data;
        }
        const matches = this.computeMatches(this.adapter?.getApp?.()?.graph, this.settings);
        if (this.settings.showOverlay) {
            this.updateOverlay(matches);
        }
        else {
            this.updateOverlay([]);
        }
        if (!matches.length) {
            this.promptMatches = [];
            return data;
        }
        const patch = this.promptPatcher.apply(data, matches);
        this.promptMatches = patch.applied;
        if (!patch.applied.length)
            return data;
        if (this.settings.showOverlay) {
            this.updateOverlay(patch.applied);
        }
        return patch.payload;
    }
    computeMatches(graph, settings) {
        return this.matcher.compute(graph, settings);
    }
    /** Read one current Comfy surface slot through the shared geometry adapter. */
    getSlotPosition(node, isOutput, slot = 0) {
        return this.graphGeometry.slotPosition(node, isOutput, slot);
    }
    refreshOverlayState({ recompute = false, graph = null, } = {}) {
        this.overlayActive = this.isOverlayEnabled();
        if (!this.overlayActive) {
            this.updateOverlay([]);
        }
        else if (recompute) {
            this.schedulePreview({ immediate: true, verbose: true, graph });
        }
    }
    schedulePreview(options = {}) {
        if (!this.overlayActive) {
            return;
        }
        const { immediate = false } = options;
        const graph = options.graph ?? this.adapter?.getApp?.()?.canvas?.graph ?? this.adapter?.getApp?.()?.graph;
        if (immediate) {
            this.runPreview({ verbose: true, reason: 'immediate', graph });
        }
        if (this.previewScheduled) {
            return;
        }
        this.previewScheduled = true;
        this.scheduler?.raf?.(() => {
            this.previewScheduled = false;
            if (!this.overlayActive) {
                return;
            }
            this.runPreview({ verbose: Boolean(options.verbose), reason: 'raf', graph });
        });
    }
    runPreview(options = {}) {
        if (!this.overlayActive) {
            return;
        }
        const graph = options.graph ?? this.adapter?.getApp?.()?.canvas?.graph ?? this.adapter?.getApp?.()?.graph;
        const matches = this.computeMatches(graph, this.settings);
        this.updateOverlay(matches);
    }
    updateOverlay(matches) {
        this.overlayMatches = Array.isArray(matches) ? matches : [];
        const signature = this.overlayMatches
            .map((match) => `${String(match.outputId)}:${String(match.outputSlot)}>` +
            `${String(match.inputId)}:${String(match.inputSlot)}`)
            .join('|');
        if (signature !== this.lastReportedMatchSignature) {
            this.lastReportedMatchSignature = signature;
            const positions = this.overlayMatches
                .map((match) => `${match.outputPos.map((value) => Math.round(value)).join(',')}>` +
                match.inputPos.map((value) => Math.round(value)).join(','))
                .join(';');
            this.logger?.debug?.(`SugarCubes resolved ${String(this.overlayMatches.length)} proximity matches` +
                `${positions ? ` at ${positions}` : ''}.`);
        }
        this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
    }
    resetOverlayState() {
        this.previewScheduled = false;
        this.promptMatches = [];
        this.updateOverlay([]);
    }
    /** Schedule the initial preview only when no preview state exists. */
    ensurePreview(graph) {
        if (this.isOverlayEnabled() && !this.previewScheduled && !this.overlayMatches.length) {
            this.schedulePreview({ immediate: true, graph });
        }
    }
    resolveSlotDirection(slot, { isOutput }) {
        if (slot && slot.dir !== undefined && slot.dir !== null) {
            return slot.dir;
        }
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        if (isOutput) {
            return liteGraph?.LinkDirection?.RIGHT ?? 4;
        }
        return liteGraph?.LinkDirection?.LEFT ?? 3;
    }
    computeDashPattern(connectionWidth, scale) {
        const safeWidth = Math.max(1, Number(connectionWidth) || 1);
        const safeScale = clamp(Number(scale) || 1, 0.2, 5);
        const dash = clamp(safeWidth * 2.8, 6 / safeScale, 48 / safeScale);
        const gap = clamp(safeWidth * 1.6, 4 / safeScale, 32 / safeScale);
        return [dash, gap];
    }
    drawProximityLinkWithRenderer(options) {
        const { ctx, canvasInstance, startPoint, endPoint, outputSlot, inputSlot, slotType, fallbackColor, linkWidthFallback, } = options;
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const renderLinkFn = typeof canvasInstance?.renderLink === 'function'
            ? canvasInstance.renderLink.bind(canvasInstance)
            : null;
        if (!renderLinkFn) {
            return false;
        }
        const startDir = this.resolveSlotDirection(outputSlot, { isOutput: true });
        const endDir = this.resolveSlotDirection(inputSlot, { isOutput: false });
        const scale = Number(canvasInstance?.ds?.scale) || 1;
        const connectionWidth = Math.max(1, Number(canvasInstance?.connections_width) || Number(linkWidthFallback) || 3);
        const dashPattern = this.computeDashPattern(connectionWidth, scale);
        const dashCycle = dashPattern.reduce((sum, value) => sum + value, 0);
        const halfDash = dashPattern[0] * 0.5;
        const approxLength = Math.hypot(endPoint.x - startPoint.x, endPoint.y - startPoint.y);
        let dashOffset = halfDash;
        if (dashCycle > 0 && Number.isFinite(approxLength)) {
            const centerPhase = (approxLength * 0.5) % dashCycle;
            dashOffset = centerPhase - halfDash;
        }
        const fakeLink = {
            id: -1,
            type: slotType,
            _pos: new Float32Array(2),
        };
        const markerNone = liteGraph?.LinkMarkerShape?.None ?? 0;
        const previousMarkerShape = canvasInstance.linkMarkerShape;
        canvasInstance.linkMarkerShape = markerNone;
        let resolvedColor = null;
        let renderOk = true;
        ctx.save();
        try {
            ctx.setLineDash(dashPattern);
            ctx.lineDashOffset = dashOffset;
            renderLinkFn(ctx, [startPoint.x, startPoint.y], [endPoint.x, endPoint.y], fakeLink, false, false, null, startDir, endDir, { disabled: false });
            if (typeof ctx.strokeStyle === 'string' && ctx.strokeStyle) {
                resolvedColor = ctx.strokeStyle;
            }
        }
        catch (error) {
            this.logger?.warn?.('SugarCubes: proximity renderLink failed', error);
            renderOk = false;
        }
        finally {
            ctx.restore();
            canvasInstance.linkMarkerShape = previousMarkerShape;
        }
        if (!renderOk) {
            return false;
        }
        if (!(typeof resolvedColor === 'string' && resolvedColor)) {
            resolvedColor = fallbackColor || canvasInstance?.default_link_color || '#7fc4ff';
        }
        return true;
    }
    render(ctx, canvasInstance) {
        if (!canvasInstance ||
            !this.overlayActive ||
            !this.overlayMatches.length ||
            this.settings.showOverlay === false) {
            return;
        }
        const radius = Number(this.settings.radius) || DEFAULT_PROXIMITY_SETTINGS.radius;
        const radiusSq = radius * radius;
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        let graphMismatchCount = 0;
        let renderedCount = 0;
        const readColor = (slotType) => {
            if (typeof canvasInstance?.getLinkColor === 'function') {
                try {
                    const value = canvasInstance.getLinkColor(slotType);
                    if (typeof value === 'string' && value) {
                        return value;
                    }
                }
                catch (error) {
                    this.logger?.warn?.('SugarCubes -> failed to resolve link color', error);
                }
            }
            if (liteGraph?.EVENT !== undefined && slotType === liteGraph.EVENT) {
                return liteGraph.EVENT_LINK_COLOR || '#AFA';
            }
            return canvasInstance?.default_link_color || '#7fc4ff';
        };
        for (const match of this.overlayMatches) {
            const outputNode = match.outputNode;
            const inputNode = match.inputNode;
            if ((outputNode && outputNode.graph !== canvasInstance.graph) ||
                (inputNode && inputNode.graph !== canvasInstance.graph)) {
                graphMismatchCount += 1;
                continue;
            }
            const slotIndexOut = match.outputSlot ?? 0;
            const slotIndexIn = match.inputSlot ?? 0;
            const outPos = match.outputPos;
            const inPos = match.inputPos;
            if (!outPos || !inPos) {
                continue;
            }
            const dx = outPos[0] - inPos[0];
            const dy = outPos[1] - inPos[1];
            if (dx * dx + dy * dy > radiusSq) {
                continue;
            }
            const outputSlot = outputNode?.outputs?.[slotIndexOut];
            const inputSlot = inputNode?.inputs?.[slotIndexIn];
            const slotType = match.outputType ?? match.inputType ?? outputSlot?.type ?? inputSlot?.type;
            const linkColor = readColor(slotType);
            const linkWidthFallback = Math.max(1, canvasInstance?.connections_width ?? 3);
            const startPoint = { x: outPos[0], y: outPos[1] };
            const endPoint = { x: inPos[0], y: inPos[1] };
            const rendered = this.drawProximityLinkWithRenderer({
                ctx,
                canvasInstance,
                startPoint,
                endPoint,
                outputSlot,
                inputSlot,
                slotType,
                fallbackColor: linkColor,
                linkWidthFallback,
            });
            if (!rendered) {
                continue;
            }
            renderedCount += 1;
        }
        const renderSignature = `${renderedCount}:${graphMismatchCount}:${this.overlayMatches.length}`;
        if (renderSignature !== this.lastReportedRenderSignature) {
            this.lastReportedRenderSignature = renderSignature;
            this.logger?.debug?.(`SugarCubes painted ${String(renderedCount)} of ` +
                `${String(this.overlayMatches.length)} proximity matches` +
                `${graphMismatchCount ? `; ${String(graphMismatchCount)} had a graph mismatch` : ''}.`);
        }
    }
}
