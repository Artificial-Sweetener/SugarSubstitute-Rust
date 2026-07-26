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
import { ProximityMatcher } from './proximity/ProximityMatcher.js';
import { ProximityPromptPatcher } from './proximity/ProximityPromptPatcher.js';
import { ProximityLinkRenderer, } from './proximity/ProximityLinkRenderer.js';
const PROXIMITY_STORAGE_KEY = 'SugarCubes.Proximity.Settings';
const DEFAULT_PROXIMITY_SETTINGS = Object.freeze({
    enabled: true,
    radius: 160,
    strict: true,
    showOverlay: true,
});
/**
 * Coordinate proximity overlay behavior for the SugarCubes UI.
 */
export class ProximityOverlay {
    adapter;
    events;
    scheduler;
    storage;
    logger;
    linkRenderer;
    matcher;
    matchSink = null;
    promptPatcher = new ProximityPromptPatcher();
    settings;
    overlayMatches;
    promptMatches;
    authoritativeMatches;
    previewScheduled;
    initializedGraphs = new WeakSet();
    overlayActive;
    lastAppliedMatchSignature = '';
    lastReportedMatchSignature = '';
    constructor({ adapter = null, events = null, scheduler = null, storage = null, } = {}) {
        this.adapter = adapter;
        this.events = events;
        this.scheduler = scheduler;
        this.storage = storage;
        this.logger = adapter?.getConsole?.() || null;
        this.linkRenderer = new ProximityLinkRenderer({
            getLiteGraph: () => this.adapter?.getLiteGraph?.() ?? null,
            logger: this.logger ?? console,
        });
        this.matcher = new ProximityMatcher({ discover: () => ({ outputs: [], inputs: [] }) }, () => this.adapter?.getLiteGraph?.() ?? null, this.logger ?? console);
        this.settings = this.loadSettings();
        this.overlayMatches = [];
        this.promptMatches = [];
        this.authoritativeMatches = [];
        this.previewScheduled = false;
        this.overlayActive = this.isProximityEnabled();
    }
    /** Replace endpoint discovery when the current graph-scoped Cube runtime changes. */
    setEndpointSource(source) {
        this.matcher = new ProximityMatcher(source, () => this.adapter?.getLiteGraph?.() ?? null, this.logger ?? console);
        this.refreshOverlayState({
            recompute: true,
            graph: this.#resolveGraph() ?? null,
        });
    }
    /** Publish authoritative matches to the graph-bound transient presentation owner. */
    setMatchSink(sink) {
        this.matchSink = sink;
        this.linkRenderer.setPositionSource(sink && typeof sink.resolveGraphPosition === 'function'
            ? sink
            : null);
        sink?.updateMatches(this.authoritativeMatches);
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
    /** Report semantic proximity activity independently from dotted-line visibility. */
    isProximityEnabled() {
        return Boolean(this.settings.enabled);
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
        this.updateOverlay(matches);
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
    refreshOverlayState({ recompute = false, graph = null, } = {}) {
        this.overlayActive = this.isProximityEnabled();
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
            if (!this.previewScheduled)
                return;
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
        if (graph && typeof graph === 'object')
            this.initializedGraphs.add(graph);
        const matches = this.computeMatches(graph, this.settings);
        this.updateOverlay(matches);
    }
    updateOverlay(matches) {
        const authoritativeMatches = Array.isArray(matches) ? matches : [];
        const appliedSignature = `${this.settings.showOverlay ? 'visible' : 'hidden'}|${matchGeometrySignature(authoritativeMatches)}`;
        if (appliedSignature === this.lastAppliedMatchSignature)
            return;
        this.lastAppliedMatchSignature = appliedSignature;
        this.authoritativeMatches = authoritativeMatches;
        this.matchSink?.updateMatches(authoritativeMatches);
        this.overlayMatches = this.settings.showOverlay ? authoritativeMatches : [];
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
        const app = this.adapter?.getApp?.();
        app?.canvas?.setDirty?.(true, true);
        app?.canvas?.graph?.setDirtyCanvas?.(true, true);
    }
    resetOverlayState() {
        this.previewScheduled = false;
        this.promptMatches = [];
        this.initializedGraphs = new WeakSet();
        this.updateOverlay([]);
    }
    /** Resolve Comfy's visible workflow graph before its root compatibility alias. */
    #resolveGraph(explicit) {
        const app = this.adapter?.getApp?.();
        return explicit ?? app?.canvas?.graph ?? app?.graph;
    }
    /** Schedule the initial preview only when no preview state exists. */
    ensurePreview(graph) {
        if (graph &&
            typeof graph === 'object' &&
            this.isProximityEnabled() &&
            !this.previewScheduled &&
            !this.initializedGraphs.has(graph)) {
            this.initializedGraphs.add(graph);
            this.schedulePreview({ immediate: true, graph });
        }
    }
    /** Render visible guides through the focused native-link renderer. */
    render(ctx, canvasInstance) {
        if (!canvasInstance ||
            !this.overlayActive ||
            !this.overlayMatches.length ||
            this.settings.showOverlay === false) {
            return;
        }
        this.linkRenderer.render(this.overlayMatches, ctx, canvasInstance);
    }
}
/** Fingerprint only state consumed by magnetic presentation and dotted rendering. */
function matchGeometrySignature(matches) {
    return matches
        .map((match) => `${String(match.outputId)}:${String(match.outputSlot)}@${match.outputPos.join(',')}>` +
        `${String(match.inputId)}:${String(match.inputSlot)}@${match.inputPos.join(',')}`)
        .join('|');
}
