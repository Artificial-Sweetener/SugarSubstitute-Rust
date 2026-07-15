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
 * Own the SugarCubes overlay rendering layer in `web/comfyui/ui/overlays/ProximityOverlay.js`.
 */
import { readWidgetValue } from '../graph/Markers.js';
import { isRecord } from '../types/common.js';
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
function normalizePortType(type) {
    if (!type || type === '*') {
        return '*';
    }
    return String(type).trim().toUpperCase();
}
function normalizeLinkId(value) {
    if (value == null) {
        return null;
    }
    if (typeof value === 'number') {
        return Number.isFinite(value) && value >= 0 ? value : null;
    }
    if (typeof value === 'string') {
        const parsed = Number(value.trim());
        return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
    }
    return null;
}
function getFirstLinkId(slot) {
    if (!slot) {
        return null;
    }
    if (Array.isArray(slot.links) && slot.links.length) {
        for (const candidate of slot.links) {
            const normalized = normalizeLinkId(candidate);
            if (normalized != null) {
                return normalized;
            }
        }
    }
    const primary = normalizeLinkId(slot.link);
    return primary != null ? primary : null;
}
function clonePromptOutput(output) {
    const result = {};
    for (const [key, value] of Object.entries(output)) {
        const entry = { ...value };
        if (Array.isArray(value.inputs)) {
            entry.inputs = value.inputs.map((input) => (Array.isArray(input) ? [...input] : input));
        }
        else if (value && typeof value.inputs === 'object') {
            const clonedInputs = {};
            for (const [inputKey, inputValue] of Object.entries(value.inputs)) {
                clonedInputs[inputKey] = Array.isArray(inputValue) ? [...inputValue] : inputValue;
            }
            entry.inputs = clonedInputs;
        }
        result[key] = entry;
    }
    return result;
}
/**
 * Coordinate proximity overlay behavior for the SugarCubes UI.
 */
export class ProximityOverlay {
    adapter;
    events;
    scheduler;
    storage;
    api;
    logger;
    settings;
    overlayMatches;
    previewScheduled;
    overlayActive;
    interceptorsInstalled;
    originalQueuePrompt;
    slotPositionBuffer = new Float32Array(2);
    constructor({ adapter = null, events = null, scheduler = null, storage = null, api = null, } = {}) {
        this.adapter = adapter;
        this.events = events;
        this.scheduler = scheduler;
        this.storage = storage;
        this.api = api;
        this.logger = adapter?.getConsole?.() || null;
        this.settings = this.loadSettings();
        this.overlayMatches = [];
        this.previewScheduled = false;
        this.overlayActive = this.isOverlayEnabled();
        this.interceptorsInstalled = false;
        this.originalQueuePrompt = null;
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
        this.persistSettings();
        this.refreshOverlayState({ recompute: true });
        return this.settings.enabled;
    }
    toggle() {
        return this.setEnabled(!this.settings.enabled);
    }
    installInterceptors() {
        if (this.interceptorsInstalled || !this.api?.queuePrompt) {
            return;
        }
        this.originalQueuePrompt = this.api.queuePrompt;
        this.api.queuePrompt = (position, payload) => {
            if (!this.settings.enabled) {
                return this.originalQueuePrompt?.call(this.api, position, payload) ?? Promise.resolve(null);
            }
            let patchedPayload = payload;
            try {
                patchedPayload = this.applyProximityToPrompt(payload);
            }
            catch (error) {
                this.logger?.error?.('SugarCubes proximity bridge failed', error);
            }
            return (this.originalQueuePrompt?.call(this.api, position, patchedPayload) ?? Promise.resolve(null));
        };
        this.interceptorsInstalled = true;
    }
    applyProximityToPrompt(data) {
        if (!this.settings.enabled) {
            this.updateOverlay([]);
            return data;
        }
        if (!isRecord(data) || !isRecord(data.output)) {
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
            return data;
        }
        const promptData = data;
        const clonedOutput = clonePromptOutput(promptData.output);
        const applied = [];
        for (const match of matches) {
            if (match.originId == null || match.originSlot == null) {
                continue;
            }
            const nodeEntry = clonedOutput[String(match.inputId)];
            if (!nodeEntry) {
                continue;
            }
            const originTuple = [String(match.originId), match.originSlot];
            if (Array.isArray(nodeEntry.inputs)) {
                const existing = nodeEntry.inputs[match.inputSlot];
                if (Array.isArray(existing) && existing.length === 2) {
                    continue;
                }
                nodeEntry.inputs[match.inputSlot] = originTuple;
                applied.push({ ...match, originTuple });
                continue;
            }
            if (nodeEntry.inputs == null) {
                nodeEntry.inputs = {};
            }
            if (typeof nodeEntry.inputs === 'object') {
                const inputKey = match.inputName || Object.keys(nodeEntry.inputs)[match.inputSlot] || 'value';
                const existing = nodeEntry.inputs[inputKey];
                if (Array.isArray(existing) && existing.length === 2) {
                    continue;
                }
                nodeEntry.inputs[inputKey] = originTuple;
                applied.push({ ...match, originTuple, inputKey });
            }
        }
        if (!applied.length) {
            return data;
        }
        if (this.settings.showOverlay) {
            this.updateOverlay(applied);
        }
        return {
            ...data,
            output: clonedOutput,
        };
    }
    computeMatches(graph, settings) {
        if (!graph || !Array.isArray(graph._nodes) || !settings) {
            return [];
        }
        const radius = Number(settings.radius) || DEFAULT_PROXIMITY_SETTINGS.radius;
        const radiusSq = radius * radius;
        const strict = Boolean(settings.strict);
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const outputs = [];
        const inputs = [];
        for (const node of graph._nodes) {
            if (!node?.type) {
                continue;
            }
            if (node.type === 'SugarCubes.CubeOutput') {
                if (this.hasExplicitOutputLink(node)) {
                    continue;
                }
                const source = this.resolveOutputSource(node);
                if (!source) {
                    continue;
                }
                const snapshot = this.buildMarkerSnapshot(node, true);
                outputs.push({
                    id: node.id,
                    cube: snapshot.cube,
                    instanceId: snapshot.instanceId,
                    alias: snapshot.alias,
                    type: snapshot.type,
                    slotPos: snapshot.slotPos,
                    slotName: snapshot.slotName,
                    node,
                    originId: source.origin_id,
                    originSlot: source.origin_slot,
                });
            }
            else if (node.type === 'SugarCubes.CubeInput') {
                if (this.hasExplicitInputLink(node)) {
                    continue;
                }
                const snapshot = this.buildMarkerSnapshot(node, false);
                inputs.push({
                    id: node.id,
                    cube: snapshot.cube,
                    instanceId: snapshot.instanceId,
                    alias: snapshot.alias,
                    type: snapshot.type,
                    slotPos: snapshot.slotPos,
                    slotName: snapshot.slotName,
                    node,
                });
            }
        }
        if (!outputs.length || !inputs.length) {
            return [];
        }
        const candidates = [];
        for (const out of outputs) {
            if (out.originId == null || out.originSlot == null) {
                continue;
            }
            for (const input of inputs) {
                if (!out.slotPos || !input.slotPos) {
                    continue;
                }
                if (this.isSameInstanceOrCube(out, input)) {
                    continue;
                }
                const dx = out.slotPos[0] - input.slotPos[0];
                const dy = out.slotPos[1] - input.slotPos[1];
                const distanceSq = dx * dx + dy * dy;
                if (distanceSq > radiusSq) {
                    continue;
                }
                if (!this.isTypePairCompatible(out.type, input.type, strict, liteGraph)) {
                    continue;
                }
                const aliasMatch = Boolean(out.alias && input.alias && out.alias === input.alias);
                const typeMatch = normalizePortType(out.type) === normalizePortType(input.type);
                const score = Math.sqrt(distanceSq) - (aliasMatch ? 40 : 0) - (typeMatch ? 20 : 0);
                candidates.push({
                    score,
                    out,
                    input,
                    distanceSq,
                });
            }
        }
        candidates.sort((a, b) => a.score - b.score);
        const usedOutputs = new Set();
        const usedInputs = new Set();
        const matches = [];
        for (const candidate of candidates) {
            const outId = candidate.out.id;
            const inId = candidate.input.id;
            if (usedOutputs.has(outId) || usedInputs.has(inId)) {
                continue;
            }
            usedOutputs.add(outId);
            usedInputs.add(inId);
            matches.push({
                outputId: outId,
                outputCube: candidate.out.cube,
                outputSlot: 0,
                outputNode: candidate.out.node,
                outputPos: candidate.out.slotPos,
                outputType: candidate.out.type,
                inputId: inId,
                inputCube: candidate.input.cube,
                inputSlot: 0,
                inputName: candidate.input.slotName || 'value',
                inputNode: candidate.input.node,
                inputPos: candidate.input.slotPos,
                inputType: candidate.input.type,
                originId: candidate.out.originId,
                originSlot: candidate.out.originSlot,
                distance: Math.sqrt(candidate.distanceSq),
                candidateDetails: {
                    outSlotPresent: Boolean(candidate.out?.slotPos),
                    inSlotPresent: Boolean(candidate.input?.slotPos),
                    outType: candidate.out?.type,
                    inType: candidate.input?.type,
                    aliasMatch: Boolean(candidate.out?.alias && candidate.out.alias === candidate.input?.alias),
                    typeMatch: normalizePortType(candidate.out?.type) === normalizePortType(candidate.input?.type),
                },
            });
        }
        return matches;
    }
    resolveGraphLink(graph, linkId) {
        if (!graph || linkId == null) {
            return null;
        }
        if (typeof graph.getLink === 'function') {
            return graph.getLink(linkId) || null;
        }
        if (graph.links && typeof graph.links === 'object') {
            if (graph.links instanceof Map) {
                return graph.links.get(linkId) || null;
            }
            if (Array.isArray(graph.links)) {
                return graph.links.find((link) => String(link.id) === String(linkId)) || null;
            }
            return graph.links[String(linkId)] || null;
        }
        return null;
    }
    resolveOutputSource(node) {
        const graph = node?.graph ?? this.adapter?.getApp?.()?.graph;
        const preferOutput = getFirstLinkId(node?.outputs?.[0]);
        if (preferOutput != null) {
            return this.resolveGraphLink(graph, preferOutput);
        }
        const fallback = getFirstLinkId(node?.inputs?.[0]);
        if (fallback != null) {
            return this.resolveGraphLink(graph, fallback);
        }
        return null;
    }
    hasExplicitInputLink(node) {
        const slot = node?.inputs?.[0];
        if (!slot) {
            return false;
        }
        const linkId = getFirstLinkId(slot);
        if (linkId == null) {
            return false;
        }
        const graph = node?.graph ?? this.adapter?.getApp?.()?.graph;
        const link = this.resolveGraphLink(graph, linkId);
        return Boolean(link);
    }
    hasExplicitOutputLink(node) {
        const slot = node?.outputs?.[0];
        if (!slot) {
            return false;
        }
        const linkId = getFirstLinkId(slot);
        if (linkId == null) {
            return false;
        }
        const graph = node?.graph ?? this.adapter?.getApp?.()?.graph;
        const link = this.resolveGraphLink(graph, linkId);
        return Boolean(link);
    }
    readSlotName(node, isOutput) {
        const slots = isOutput ? node.outputs : node.inputs;
        if (Array.isArray(slots) && slots.length && slots[0]) {
            const slot = slots[0];
            if (typeof slot.name === 'string' && slot.name) {
                return slot.name;
            }
            if (typeof slot.label === 'string' && slot.label) {
                return slot.label;
            }
        }
        return 'value';
    }
    getSlotPosition(node, isOutput) {
        try {
            if (typeof node.getConnectionPos === 'function') {
                const temp = this.slotPositionBuffer;
                const isInput = !isOutput;
                node.getConnectionPos(isInput, 0, temp);
                if (Number.isFinite(temp[0]) && Number.isFinite(temp[1])) {
                    return [temp[0] ?? 0, temp[1] ?? 0];
                }
                const pos = node.getConnectionPos(isInput, 0);
                if (Array.isArray(pos) && pos.length === 2) {
                    return [Number(pos[0]) || 0, Number(pos[1]) || 0];
                }
                if (pos instanceof Float32Array || pos instanceof Float64Array) {
                    return [pos[0] ?? 0, pos[1] ?? 0];
                }
            }
        }
        catch (error) {
            this.logger?.warn?.('SugarCubes: failed to read connection pos', error);
        }
        const base = Array.isArray(node.pos) ? node.pos : [0, 0];
        const size = Array.isArray(node.size) ? node.size : [0, 0];
        const baseX = Number(base[0]) || 0;
        const baseY = Number(base[1]) || 0;
        const width = Number(size[0]) || 0;
        const height = Number(size[1]) || 0;
        return [isOutput ? baseX + width : baseX, baseY + height / 2];
    }
    buildMarkerSnapshot(node, isOutput) {
        const cube = readWidgetValue(node, 'cube_id');
        const instanceId = readWidgetValue(node, 'instance_id');
        const slotName = this.readSlotName(node, isOutput);
        const slotPos = this.getSlotPosition(node, isOutput);
        const slot = isOutput ? node.outputs?.[0] : node.inputs?.[0];
        const type = slot?.type ?? null;
        return {
            cube: cube || null,
            instanceId: instanceId || null,
            alias: slotName,
            slotPos,
            slotName,
            type,
            node,
            isOutput,
        };
    }
    isSameInstanceOrCube(output, input) {
        if (output?.instanceId && input?.instanceId) {
            return output.instanceId === input.instanceId;
        }
        if (output?.cube && input?.cube) {
            return output.cube === input.cube;
        }
        return false;
    }
    isTypePairCompatible(outputType, inputType, strict, liteGraph) {
        const out = normalizePortType(outputType);
        const inn = normalizePortType(inputType);
        try {
            if (liteGraph && typeof liteGraph.isValidConnection === 'function') {
                if (strict) {
                    if (out === '*' || inn === '*') {
                        return false;
                    }
                    return liteGraph.isValidConnection(out, inn) && liteGraph.isValidConnection(inn, out);
                }
                return liteGraph.isValidConnection(out || '*', inn || '*');
            }
        }
        catch (error) {
            this.logger?.debug?.('SugarCubes: type compatibility check failed', error);
        }
        if (strict) {
            if (out === '*' || inn === '*') {
                return false;
            }
            return out === inn;
        }
        if (out === '*' || inn === '*') {
            return true;
        }
        return out === inn;
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
        this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
    }
    resetOverlayState() {
        this.previewScheduled = false;
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
            if (!outputNode || !inputNode) {
                continue;
            }
            if (outputNode.graph !== canvasInstance.graph || inputNode.graph !== canvasInstance.graph) {
                continue;
            }
            const outPos = this.getSlotPosition(outputNode, true);
            const inPos = this.getSlotPosition(inputNode, false);
            if (!outPos || !inPos) {
                continue;
            }
            const dx = outPos[0] - inPos[0];
            const dy = outPos[1] - inPos[1];
            if (dx * dx + dy * dy > radiusSq) {
                continue;
            }
            const slotIndexOut = match.outputSlot ?? 0;
            const slotIndexIn = match.inputSlot ?? 0;
            const outputSlot = outputNode.outputs?.[slotIndexOut];
            const inputSlot = inputNode.inputs?.[slotIndexIn];
            const slotType = outputSlot?.type ?? inputSlot?.type;
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
        }
    }
}
