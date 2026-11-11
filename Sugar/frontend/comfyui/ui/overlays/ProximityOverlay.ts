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
import type { ComfyApplication, ComfyGraph } from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';
import { ProximityMatcher, type ProximityLiteGraph } from './proximity/ProximityMatcher.js';
import type {
  ProximityEndpointSource,
  ProximityMatch,
  ProximityMatchSink,
} from './proximity/ProximityModel.js';
import { ProximityPromptPatcher } from './proximity/ProximityPromptPatcher.js';
import {
  ProximityLinkRenderer,
  type ProximityPortPositionSource,
  type ProximityRenderCanvas,
} from './proximity/ProximityLinkRenderer.js';
export type { ProximityMatch } from './proximity/ProximityModel.js';

interface ProximitySettings extends UnknownRecord {
  enabled: boolean;
  radius: number;
  strict: boolean;
  showOverlay: boolean;
}
type ProximityGraph = ComfyGraph;
interface ProximityAdapter {
  getConsole?(): Console | null;
  getApp?(): ComfyApplication | null;
  getLiteGraph?(): ProximityLiteGraph | null;
}
interface ProximityStorage {
  readJson?(key: string): UnknownRecord | null;
  writeJson?(key: string, value: unknown): void;
}
interface ProximityScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}
interface ProximityOptions {
  adapter?: ProximityAdapter | null;
  events?: unknown;
  scheduler?: ProximityScheduler | null;
  storage?: ProximityStorage | null;
}
interface PreviewOptions {
  immediate?: boolean;
  verbose?: boolean;
  graph?: ProximityGraph | null | undefined;
  reason?: string;
}

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
  private readonly adapter: ProximityAdapter | null;
  readonly events: unknown;
  private readonly scheduler: ProximityScheduler | null;
  private readonly storage: ProximityStorage | null;
  private readonly logger: Console | null;
  private readonly linkRenderer: ProximityLinkRenderer;
  private matcher: ProximityMatcher;
  private matchSink: ProximityMatchSink | null = null;
  private readonly promptPatcher = new ProximityPromptPatcher();
  settings: ProximitySettings;
  overlayMatches: ProximityMatch[];
  promptMatches: ProximityMatch[];
  private authoritativeMatches: ProximityMatch[];
  private previewScheduled: boolean;
  private initializedGraphs = new WeakSet<object>();
  overlayActive: boolean;
  private lastAppliedMatchSignature = '';
  private lastReportedMatchSignature = '';

  constructor({
    adapter = null,
    events = null,
    scheduler = null,
    storage = null,
  }: ProximityOptions = {}) {
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.storage = storage;
    this.logger = adapter?.getConsole?.() || null;
    this.linkRenderer = new ProximityLinkRenderer({
      getLiteGraph: () => this.adapter?.getLiteGraph?.() ?? null,
      logger: this.logger ?? console,
    });
    this.matcher = new ProximityMatcher(
      { discover: () => ({ outputs: [], inputs: [] }) },
      () => this.adapter?.getLiteGraph?.() ?? null,
      this.logger ?? console,
    );
    this.settings = this.loadSettings();
    this.overlayMatches = [];
    this.promptMatches = [];
    this.authoritativeMatches = [];
    this.previewScheduled = false;
    this.overlayActive = this.isProximityEnabled();
  }

  /** Replace endpoint discovery when the current graph-scoped Cube runtime changes. */
  setEndpointSource(source: ProximityEndpointSource): void {
    this.matcher = new ProximityMatcher(
      source,
      () => this.adapter?.getLiteGraph?.() ?? null,
      this.logger ?? console,
    );
    this.refreshOverlayState({
      recompute: true,
      graph: this.#resolveGraph() ?? null,
    });
  }

  /** Publish authoritative matches to the graph-bound transient presentation owner. */
  setMatchSink(sink: (ProximityMatchSink & Partial<ProximityPortPositionSource>) | null): void {
    this.matchSink = sink;
    this.linkRenderer.setPositionSource(
      sink && typeof sink.resolveGraphPosition === 'function'
        ? (sink as ProximityPortPositionSource)
        : null,
    );
    sink?.updateMatches(this.authoritativeMatches);
  }

  loadSettings(): ProximitySettings {
    try {
      const stored = this.storage?.readJson?.(PROXIMITY_STORAGE_KEY);
      if (!stored || typeof stored !== 'object') {
        return { ...DEFAULT_PROXIMITY_SETTINGS };
      }
      return {
        enabled:
          typeof stored.enabled === 'boolean' ? stored.enabled : DEFAULT_PROXIMITY_SETTINGS.enabled,
        radius:
          typeof stored.radius === 'number' ? stored.radius : DEFAULT_PROXIMITY_SETTINGS.radius,
        strict:
          typeof stored.strict === 'boolean' ? stored.strict : DEFAULT_PROXIMITY_SETTINGS.strict,
        showOverlay:
          typeof stored.showOverlay === 'boolean'
            ? stored.showOverlay
            : DEFAULT_PROXIMITY_SETTINGS.showOverlay,
      };
    } catch (error) {
      this.logger?.warn?.('SugarCubes: failed to load proximity settings', error);
      return { ...DEFAULT_PROXIMITY_SETTINGS };
    }
  }

  persistSettings(): void {
    try {
      this.storage?.writeJson?.(PROXIMITY_STORAGE_KEY, this.settings);
    } catch (error) {
      this.logger?.warn?.('SugarCubes: failed to persist proximity settings', error);
    }
  }

  /** Report semantic proximity activity independently from dotted-line visibility. */
  isProximityEnabled(): boolean {
    return Boolean(this.settings.enabled);
  }

  setEnabled(enabled: boolean): boolean {
    this.settings.enabled = Boolean(enabled);
    if (!this.settings.enabled) {
      this.promptMatches = [];
    }
    this.persistSettings();
    this.refreshOverlayState({ recompute: true });
    return this.settings.enabled;
  }

  toggle(): boolean {
    return this.setEnabled(!this.settings.enabled);
  }

  applyProximityToPrompt(data: unknown): unknown {
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
    if (!patch.applied.length) return data;

    if (this.settings.showOverlay) {
      this.updateOverlay(patch.applied);
    }
    return patch.payload;
  }

  /** Resolve fresh accepted edges for SugarCubes-owned direct execution. */
  resolveExecutionMatches(graph?: ProximityGraph | null): readonly ProximityMatch[] {
    if (!this.settings.enabled) {
      this.updateOverlay([]);
      return [];
    }
    if (this.authoritativeMatches.length) return this.authoritativeMatches;
    const matches = this.computeMatches(
      graph ?? this.adapter?.getApp?.()?.canvas?.graph ?? this.adapter?.getApp?.()?.graph,
      this.settings,
    );
    this.updateOverlay(matches);
    return matches;
  }

  /** Return endpoint-only state for opt-in host diagnostics. */
  executionDebugState(): UnknownRecord {
    return {
      enabled: this.settings.enabled,
      authoritative: this.authoritativeMatches.map(describeMatch),
      overlay: this.overlayMatches.map(describeMatch),
      prompt: this.promptMatches.map(describeMatch),
    };
  }

  computeMatches(
    graph: ProximityGraph | null | undefined,
    settings: Partial<ProximitySettings>,
  ): ProximityMatch[] {
    return this.matcher.compute(graph, settings);
  }

  refreshOverlayState({
    recompute = false,
    graph = null,
  }: { recompute?: boolean; graph?: ProximityGraph | null } = {}): void {
    this.overlayActive = this.isProximityEnabled();
    if (!this.overlayActive) {
      this.updateOverlay([]);
    } else if (recompute) {
      this.schedulePreview({ immediate: true, verbose: true, graph });
    }
  }

  schedulePreview(options: PreviewOptions = {}): void {
    if (!this.overlayActive) {
      return;
    }
    const { immediate = false } = options;
    const graph =
      options.graph ?? this.adapter?.getApp?.()?.canvas?.graph ?? this.adapter?.getApp?.()?.graph;
    if (immediate) {
      this.runPreview({ verbose: true, reason: 'immediate', graph });
      if (!this.previewScheduled) return;
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

  runPreview(options: PreviewOptions = {}): void {
    if (!this.overlayActive) {
      return;
    }
    const graph =
      options.graph ?? this.adapter?.getApp?.()?.canvas?.graph ?? this.adapter?.getApp?.()?.graph;
    if (graph && typeof graph === 'object') this.initializedGraphs.add(graph);
    const matches = this.computeMatches(graph, this.settings);
    this.updateOverlay(matches);
  }

  updateOverlay(matches: ProximityMatch[]): void {
    const authoritativeMatches = Array.isArray(matches) ? matches : [];
    const appliedSignature = `${this.settings.showOverlay ? 'visible' : 'hidden'}|${matchGeometrySignature(authoritativeMatches)}`;
    if (appliedSignature === this.lastAppliedMatchSignature) return;
    this.lastAppliedMatchSignature = appliedSignature;
    this.authoritativeMatches = authoritativeMatches;
    this.matchSink?.updateMatches(authoritativeMatches);
    this.overlayMatches = this.settings.showOverlay ? authoritativeMatches : [];
    const signature = this.overlayMatches
      .map(
        (match) =>
          `${String(match.outputId)}:${String(match.outputSlot)}>` +
          `${String(match.inputId)}:${String(match.inputSlot)}`,
      )
      .join('|');
    if (signature !== this.lastReportedMatchSignature) {
      this.lastReportedMatchSignature = signature;
      const positions = this.overlayMatches
        .map(
          (match) =>
            `${match.outputPos.map((value) => Math.round(value)).join(',')}>` +
            match.inputPos.map((value) => Math.round(value)).join(','),
        )
        .join(';');
      this.logger?.debug?.(
        `SugarCubes resolved ${String(this.overlayMatches.length)} proximity matches` +
          `${positions ? ` at ${positions}` : ''}.`,
      );
    }
    const app = this.adapter?.getApp?.();
    app?.canvas?.setDirty?.(true, true);
    app?.canvas?.graph?.setDirtyCanvas?.(true, true);
  }

  resetOverlayState(): void {
    this.previewScheduled = false;
    this.promptMatches = [];
    this.initializedGraphs = new WeakSet<object>();
    this.updateOverlay([]);
  }

  /** Resolve Comfy's visible workflow graph before its root compatibility alias. */
  #resolveGraph(explicit?: ProximityGraph | null): ProximityGraph | null | undefined {
    const app = this.adapter?.getApp?.();
    return explicit ?? app?.canvas?.graph ?? app?.graph;
  }

  /** Schedule the initial preview only when no preview state exists. */
  ensurePreview(graph: ComfyGraph | null | undefined): void {
    if (
      graph &&
      typeof graph === 'object' &&
      this.isProximityEnabled() &&
      !this.previewScheduled &&
      !this.initializedGraphs.has(graph)
    ) {
      this.initializedGraphs.add(graph);
      this.schedulePreview({ immediate: true, graph });
    }
  }

  /** Render visible guides through the focused native-link renderer. */
  render(ctx: CanvasRenderingContext2D, canvasInstance: ProximityRenderCanvas): void {
    if (
      !canvasInstance ||
      !this.overlayActive ||
      !this.overlayMatches.length ||
      this.settings.showOverlay === false
    ) {
      return;
    }
    this.linkRenderer.render(this.overlayMatches, ctx, canvasInstance);
  }
}

/** Fingerprint only state consumed by magnetic presentation and dotted rendering. */
function matchGeometrySignature(matches: readonly ProximityMatch[]): string {
  return matches
    .map(
      (match) =>
        `${String(match.outputId)}:${String(match.outputSlot)}@${match.outputPos.join(',')}>` +
        `${String(match.inputId)}:${String(match.inputSlot)}@${match.inputPos.join(',')}`,
    )
    .join('|');
}

/** Strip one accepted match to stable non-sensitive endpoint facts. */
function describeMatch(match: ProximityMatch): UnknownRecord {
  return {
    output_id: match.outputId,
    output_instance_id: match.outputInstanceId,
    output_binding: match.outputBinding,
    input_id: match.inputId,
    input_instance_id: match.inputInstanceId,
    input_binding: match.inputBinding,
  };
}
