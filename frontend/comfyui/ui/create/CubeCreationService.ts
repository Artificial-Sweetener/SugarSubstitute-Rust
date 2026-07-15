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
/** Own discovery, preview, and persistence for new personal SugarCubes. */

import { isCanonicalCubeId, normalizeDefaultAliasTitle } from '../core/CubeId.js';
import {
  normalizeCubeRoute,
  normalizeSupportedModels,
  normalizeTargetModel,
} from '../core/ModelTargets.js';
import { resolveNewInstanceBounds } from '../graph/CubeBounds.js';
import { isCubeMarkerType, updateMarkersForIds } from '../graph/CubeMarkers.js';
import { buildLinkIndex, getGraphNodes } from '../graph/GraphQuery.js';
import { readWidgetValue, writeWidgetValue } from '../graph/Markers.js';
import { enrichWorkflowPayload } from '../graph/WorkflowPayloadBuilder.js';
import {
  SugarCubeExportError,
  buildErrorDetail,
  formatSaveSummaryEntry,
  formatViolations,
} from '../save/SaveFeedback.js';
import { suggestPersonalCubeIdentity } from './PersonalCubeIdentity.js';
import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { CubeBoundsPayload } from '../graph/CubeBounds.js';
import type { SavedCubeResult } from '../save/CubeSaveReconciler.js';
import type { UnknownRecord } from '../types/common.js';
import type { CreatePersonalCubeModalOptions } from '../dialogs/CreatePersonalCubeModal.js';
import type {
  ComfyApplication,
  ComfyCanvas,
  ComfyGraph,
  ComfyNode,
  GraphId,
} from '../types/graph.js';

const CREATE_MARKER_TYPES = new Set(['SugarCubes.CubeInput', 'SugarCubes.CubeOutput']);

export interface CreateCubeCandidate {
  graph: ComfyGraph;
  markerNode: ComfyNode;
  defaultAlias: string;
  targetModel: string;
  supportedModels: string[];
  cubeId: string;
  filename: string;
  markerIds: GraphId[];
  nodeIds: GraphId[];
  markers: ComfyNode[];
  nodes: ComfyNode[];
  warnings: unknown[];
  description: string;
}

export interface CreateCubeValues {
  targetModel?: unknown;
  defaultAlias?: unknown;
  cubeName?: unknown;
  name?: unknown;
  cubeId?: unknown;
  supportedModels?: unknown;
  description?: unknown;
}

export type SaveCubeCandidate = Pick<
  CreateCubeCandidate,
  'graph' | 'filename' | 'markerIds' | 'markers'
>;

export type PreviewCubeCandidate = Pick<
  CreateCubeCandidate,
  'defaultAlias' | 'graph' | 'markers' | 'nodes'
>;

export interface CreatedCubeResult {
  cubeId: string;
  defaultAlias: string;
  targetModel: string;
  supportedModels: string[];
  summary: string;
  saved: SavedCubeResult[];
}

interface CreationAdapter {
  getApp?(): ComfyApplication | null;
  getCanvas?(): ComfyCanvas | null;
  getWindow?(): Window | null;
  getConsole?(): Pick<Console, 'error'> | null;
}

export interface CreationApi {
  saveImplementation(payload: string, options?: RequestInit): Promise<ApiJsonResult>;
}

interface CreationToast {
  push?(severity: string, summary: string, detail?: string): unknown;
}

interface CreationInstanceManager {
  refresh?(options: { graph: ComfyGraph; reason: string }): unknown;
  scheduleRefresh?(options?: unknown): unknown;
}

interface CreationBrowser {
  getCubes?(): UnknownRecord[];
  refresh?(options: { force: boolean }): Promise<unknown>;
}

interface CreationDialogs {
  openCreatePersonalCube?(
    options: CreatePersonalCubeModalOptions,
  ): Promise<CreateCubeValues | null>;
}

interface CreationSaveReconciler {
  reconcile(options: {
    graph: ComfyGraph;
    saved: SavedCubeResult[];
    fallbackCubeIds: unknown[];
    markerIdsByCubeId: Record<string, GraphId[]>;
    reason: string;
  }): Promise<unknown>;
}

export interface CubeCreationDependencies {
  adapter: CreationAdapter;
  api?: CreationApi | null;
  toast?: CreationToast | null;
  instanceManager?: CreationInstanceManager | null;
  cubeBrowser?: CreationBrowser | null;
  dialogs?: CreationDialogs | null;
  saveReconciler?: CreationSaveReconciler | null;
}

interface MarkerWidgetState {
  marker: ComfyNode;
  values: Record<string, unknown>;
}

interface CreateCandidateCollection {
  markerIds: GraphId[];
  nodeIds: GraphId[];
  markers: ComfyNode[];
  nodes: ComfyNode[];
  warnings: unknown[];
}

/** Coordinate the zero-setup personal cube creation use case. */
export class CubeCreationService {
  private readonly adapter: CreationAdapter;
  private readonly api: CreationApi | null;
  private readonly toast: CreationToast | null;
  private readonly instanceManager: CreationInstanceManager | null;
  private readonly cubeBrowser: CreationBrowser | null;
  private readonly dialogs: CreationDialogs | null;
  private readonly saveReconciler: CreationSaveReconciler | null;
  private previewRestorer: (() => void) | null;

  constructor({
    adapter,
    api = null,
    toast = null,
    instanceManager = null,
    cubeBrowser = null,
    dialogs = null,
    saveReconciler = null,
  }: CubeCreationDependencies) {
    this.adapter = adapter;
    this.api = api;
    this.toast = toast;
    this.instanceManager = instanceManager;
    this.cubeBrowser = cubeBrowser;
    this.dialogs = dialogs;
    this.saveReconciler = saveReconciler;
    this.previewRestorer = null;
  }

  /** Discover one marker-connected cube and guide the name-only local save. */
  async startCreateCubeFromMarker(
    markerNode: ComfyNode | null | undefined,
  ): Promise<CreatedCubeResult | null> {
    try {
      const candidate = this.analyzeCreateCandidateFromMarker(markerNode);
      this.startCreateCubePreview(candidate);
      const existingCubeIds = (this.cubeBrowser?.getCubes?.() || [])
        .map((entry) => (typeof entry?.cube_id === 'string' ? entry.cube_id : ''))
        .filter(Boolean);
      const values = await this.dialogs?.openCreatePersonalCube?.({
        candidate,
        deriveIdentity: (name: string) => suggestPersonalCubeIdentity(name, existingCubeIds),
      });
      if (!values) return null;
      const result = await this.saveCreatedCubeCandidate(candidate, values);
      this.toast?.push?.('success', 'SugarCube created', result.summary);
      return result;
    } catch (error: unknown) {
      const exportError = SugarCubeExportError.from(error);
      this.toast?.push?.(
        'error',
        exportError.message,
        exportError.detail || formatViolations(exportError.violations),
      );
      this.adapter?.getConsole?.()?.error?.(exportError.message);
      return null;
    } finally {
      this.clearCreateCubePreview();
    }
  }

  /** Return the executable and boundary nodes connected to one empty marker. */
  analyzeCreateCandidateFromMarker(markerNode: ComfyNode | null | undefined): CreateCubeCandidate {
    const graph = markerNode?.graph ?? this.adapter?.getApp?.()?.graph;
    if (!graph || !markerNode || !isCubeMarkerType(markerNode)) {
      throw new Error('Marker unavailable.');
    }
    if (String(readWidgetValue(markerNode, 'cube_id') || '').trim()) {
      throw new Error('This SugarCube is already initialized.');
    }
    const defaultAlias = this.readMarkerDefaultAlias(markerNode);
    const candidate = this.collectCreateCandidateFromMarker(graph, markerNode, defaultAlias);
    if (!candidate.nodeIds.length) {
      throw new Error('No nodes connected to this marker.');
    }
    const identity = suggestPersonalCubeIdentity(defaultAlias || 'SugarCube');
    return {
      graph,
      markerNode,
      defaultAlias,
      targetModel: '',
      supportedModels: [],
      cubeId: identity.cubeId,
      filename: identity.cubeId.split('/').pop() || 'cube.cube',
      ...candidate,
      description: '',
    };
  }

  /** Normalize one marker's authored default alias. */
  readMarkerDefaultAlias(markerNode: ComfyNode | null | undefined): string {
    return normalizeDefaultAliasTitle(readWidgetValue(markerNode, 'default_alias'));
  }

  /** Traverse in marker direction until matching cube-marker boundaries. */
  collectCreateCandidateFromMarker(
    graph: ComfyGraph,
    markerNode: ComfyNode,
    defaultAliasKey: string,
  ): CreateCandidateCollection {
    if (!markerNode.type || !CREATE_MARKER_TYPES.has(markerNode.type)) {
      throw new Error('Create cube starts from a cube input or output marker.');
    }
    const allNodes = getGraphNodes(graph);
    const nodesById = new Map(
      allNodes.filter((node) => node?.id != null).map((node) => [String(node.id), node]),
    );
    const { outgoing, incoming } = buildLinkIndex(graph);
    const visited = new Set([String(markerNode.id)]);
    const queue: string[] = [];
    const markerIds = new Set<string>([String(markerNode.id)]);
    const executableIds = new Set<string>();
    const direction = markerNode.type === 'SugarCubes.CubeOutput' ? 'incoming' : 'outgoing';
    const enqueue = (nodeId: GraphId | null | undefined): void => {
      const key = String(nodeId ?? '');
      if (!key || visited.has(key)) return;
      visited.add(key);
      const node = nodesById.get(key);
      if (!node) return;
      if (isCubeMarkerType(node)) {
        if (
          Boolean(node.type && CREATE_MARKER_TYPES.has(node.type)) &&
          this.readMarkerDefaultAlias(node) === defaultAliasKey
        ) {
          markerIds.add(key);
        }
        return;
      }
      executableIds.add(key);
      queue.push(key);
    };
    const nextId = (edge: {
      target_id?: GraphId | null;
      target?: GraphId | null;
      origin_id?: GraphId | null;
      origin?: GraphId | null;
    }) =>
      direction === 'outgoing' ? (edge.target_id ?? edge.target) : (edge.origin_id ?? edge.origin);
    const startEdges =
      direction === 'outgoing'
        ? outgoing.get(String(markerNode.id))
        : incoming.get(String(markerNode.id));
    for (const edge of startEdges || []) enqueue(nextId(edge));
    while (queue.length) {
      const current = queue.shift();
      if (!current) continue;
      const edges = direction === 'outgoing' ? outgoing.get(current) : incoming.get(current);
      for (const edge of edges || []) enqueue(nextId(edge));
    }
    const markers: ComfyNode[] = [];
    const nodes: ComfyNode[] = [];
    const resolvedMarkerIds: GraphId[] = [];
    const nodeIds: GraphId[] = [];
    for (const node of allNodes) {
      const key = String(node?.id);
      if (markerIds.has(key)) {
        markers.push(node);
        if (node.id != null) resolvedMarkerIds.push(node.id);
      } else if (executableIds.has(key)) {
        nodes.push(node);
        if (node.id != null) nodeIds.push(node.id);
      }
    }
    return { markerIds: resolvedMarkerIds, nodeIds, markers, nodes, warnings: [] };
  }

  /** Draw a transient managed-group preview without mutating serialized graph state. */
  startCreateCubePreview(candidate: PreviewCubeCandidate | null | undefined): void {
    this.clearCreateCubePreview();
    if (!candidate || ![...candidate.nodes, ...candidate.markers].some(Boolean)) return;
    const bounds = resolveNewInstanceBounds({
      nodes: candidate?.nodes || [],
      markers: candidate?.markers || [],
    });
    const canvas = this.resolveCreatePreviewCanvas();
    const previousBackground = canvas?.onDrawBackground || null;
    let previewBackground = null;
    if (canvas) {
      previewBackground = (ctx: CanvasRenderingContext2D, ...args: unknown[]) => {
        previousBackground?.call(canvas, ctx, ...args);
        this.drawCreatePreviewGroup(ctx, canvas, candidate, bounds);
      };
      canvas.onDrawBackground = previewBackground;
    }
    candidate?.graph?.setDirtyCanvas?.(true, true);
    canvas?.setDirty?.(true, true);
    this.previewRestorer = () => {
      if (canvas?.onDrawBackground === previewBackground)
        canvas.onDrawBackground = previousBackground;
      candidate?.graph?.setDirtyCanvas?.(true, true);
      canvas?.setDirty?.(true, true);
    };
  }

  /** Resolve the active LiteGraph canvas across supported ComfyUI hosts. */
  resolveCreatePreviewCanvas(): ComfyCanvas | null {
    const windowRef = this.adapter?.getWindow?.() || globalThis.window;
    const windowApp = isRecord(windowRef?.app) ? (windowRef.app as ComfyApplication) : null;
    const appRef = this.adapter.getApp?.() || windowApp;
    const canvasHost = isRecord(windowRef?.LGraphCanvas) ? windowRef.LGraphCanvas : {};
    const activeCanvas = isComfyCanvas(canvasHost.active_canvas) ? canvasHost.active_canvas : null;
    return (
      appRef?.canvas || appRef?.graph?.canvas || this.adapter?.getCanvas?.() || activeCanvas || null
    );
  }

  /** Render the transient preview rectangle. */
  drawCreatePreviewGroup(
    ctx: CanvasRenderingContext2D | null | undefined,
    graphCanvas: ComfyCanvas,
    candidate: PreviewCubeCandidate,
    bounds: CubeBoundsPayload | null,
  ): void {
    if (!ctx || !bounds) return;
    const fontSize = 24;
    const alpha = Number(graphCanvas?.editor_alpha) || 1;
    ctx.save();
    ctx.globalAlpha = 0.25 * alpha;
    ctx.fillStyle = '#f7c948';
    ctx.strokeStyle = '#f7c948';
    ctx.beginPath();
    ctx.rect(bounds.x + 0.5, bounds.y + 0.5, bounds.w, fontSize * 1.4);
    ctx.fill();
    ctx.beginPath();
    ctx.rect(bounds.x + 0.5, bounds.y + 0.5, bounds.w, bounds.h);
    ctx.fill();
    ctx.globalAlpha = alpha;
    ctx.stroke();
    ctx.font = `${fontSize}px Arial`;
    ctx.textAlign = 'left';
    ctx.fillText(
      `${candidate?.defaultAlias || 'SugarCube'} Preview`,
      bounds.x + 4,
      bounds.y + fontSize,
    );
    ctx.restore();
  }

  /** Remove the transient preview and restore the host draw callback. */
  clearCreateCubePreview(): void {
    const restore = this.previewRestorer;
    this.previewRestorer = null;
    restore?.();
  }

  /** Persist one confirmed candidate and reconcile backend-finalized definitions. */
  async saveCreatedCubeCandidate(
    candidate: SaveCubeCandidate,
    values: CreateCubeValues,
  ): Promise<CreatedCubeResult> {
    const graph = candidate?.graph;
    if (!graph) throw new Error('Unable to access the current graph');
    const targetModel = normalizeTargetModel(values?.targetModel);
    const requestedAlias = normalizeCubeRoute(values?.defaultAlias || '');
    const shortName = normalizeDefaultAliasTitle(
      values?.cubeName || values?.name || values?.defaultAlias,
    );
    const defaultAlias = requestedAlias || shortName || 'SugarCube';
    const cubeId = typeof values?.cubeId === 'string' ? values.cubeId.trim() : '';
    const supportedModels = normalizeSupportedModels(values?.supportedModels, { targetModel });
    if (!defaultAlias || !isCanonicalCubeId(cubeId)) {
      throw new Error('A valid default alias and canonical cube id are required.');
    }
    const markerState = this.captureMarkerWidgetState(candidate.markers);
    try {
      updateMarkersForIds(graph, candidate.markerIds, {
        cubeId,
        defaultAlias,
        instanceAlias: defaultAlias,
      });
      this.instanceManager?.refresh?.({ graph, reason: 'cube-create' });
      const promptResult = this.adapter?.getApp?.()?.graphToPrompt?.();
      const resolved: unknown = isPromiseLike(promptResult) ? await promptResult : promptResult;
      const resolvedRecord = isRecord(resolved) ? resolved : {};
      const graphPayload = resolvedRecord.output ?? resolvedRecord.prompt ?? resolved;
      if (!isRecord(graphPayload)) throw new Error('Unable to serialize the current graph');
      const workflowPayload = resolvedRecord.workflow;
      if (!workflowPayload || !isRecord(workflowPayload) || Array.isArray(workflowPayload)) {
        throw new Error('Workflow payload unavailable');
      }
      const workflow = enrichWorkflowPayload(workflowPayload, graph);
      const requestBody = {
        graph: graphPayload,
        cubes: [
          {
            cube_id: cubeId,
            forked: false,
            lineage: null,
            previous_cube_id: '',
            description: typeof values?.description === 'string' ? values.description : '',
            metadata: {
              default_alias: defaultAlias,
              ...(targetModel ? { target_model: targetModel } : {}),
              ...(supportedModels.length ? { supported_models: supportedModels } : {}),
            },
          },
        ],
        workflow,
        workflow_version: workflow?.version ?? null,
      };
      if (!this.api) throw new Error('Cube creation API is unavailable');
      const { response, data } = await this.api.saveImplementation(JSON.stringify(requestBody), {
        headers: { 'Content-Type': 'application/json' },
      });
      if (!response.ok || data.error) {
        const apiError = isRecord(data.error) ? data.error : {};
        throw new SugarCubeExportError(
          typeof apiError.message === 'string'
            ? apiError.message
            : response.statusText || 'Export failed',
          buildErrorDetail(apiError),
          apiError.violations,
        );
      }
      const saved = Array.isArray(data.saved)
        ? data.saved.filter((entry): entry is SavedCubeResult => isRecord(entry))
        : [];
      const summary = saved.length
        ? saved.map((entry) => formatSaveSummaryEntry(toSaveSummaryEntry(entry))).join('\n')
        : `saved only: ${defaultAlias} -> ${candidate.filename}`;
      if (!this.saveReconciler?.reconcile) throw new Error('Cube save reconciler is unavailable');
      await this.saveReconciler.reconcile({
        graph,
        saved,
        fallbackCubeIds: saved.map((entry) => entry?.cube_id).filter(Boolean).length
          ? saved.map((entry) => entry?.cube_id).filter(Boolean)
          : [cubeId],
        markerIdsByCubeId: { [cubeId]: candidate.markerIds },
        reason: 'cube-create',
      });
      void this.cubeBrowser?.refresh?.({ force: true }).catch(() => {});
      return { cubeId, defaultAlias, targetModel, supportedModels, summary, saved };
    } catch (error: unknown) {
      this.restoreMarkerWidgetState(markerState);
      this.instanceManager?.refresh?.({ graph, reason: 'cube-create-rollback' });
      throw error;
    }
  }

  /** Capture marker widgets for rollback around graph serialization failures. */
  captureMarkerWidgetState(markers: readonly ComfyNode[]): MarkerWidgetState[] {
    return (Array.isArray(markers) ? markers : []).map((marker) => ({
      marker,
      values: Object.fromEntries(
        ['cube_id', 'default_alias', 'instance_alias', 'instance_id'].map((name) => [
          name,
          readWidgetValue(marker, name),
        ]),
      ),
    }));
  }

  /** Restore captured marker widgets after a failed creation save. */
  restoreMarkerWidgetState(entries: readonly MarkerWidgetState[]): void {
    for (const entry of Array.isArray(entries) ? entries : []) {
      for (const [name, value] of Object.entries(entry.values || {})) {
        writeWidgetValue(entry.marker, name, value);
      }
    }
  }
}

function isComfyCanvas(value: unknown): value is ComfyCanvas {
  return isRecord(value);
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  return isRecord(value) && typeof value.then === 'function';
}

function toSaveSummaryEntry(entry: SavedCubeResult): {
  committed?: boolean;
  default_alias?: string;
  path?: string;
  commit_short_sha?: string;
  commit_message?: string;
  commit_error?: string;
} {
  return {
    ...(typeof entry.committed === 'boolean' ? { committed: entry.committed } : {}),
    ...(typeof entry.default_alias === 'string' ? { default_alias: entry.default_alias } : {}),
    ...(typeof entry.path === 'string' ? { path: entry.path } : {}),
    ...(typeof entry.commit_short_sha === 'string'
      ? { commit_short_sha: entry.commit_short_sha }
      : {}),
    ...(typeof entry.commit_message === 'string' ? { commit_message: entry.commit_message } : {}),
    ...(typeof entry.commit_error === 'string' ? { commit_error: entry.commit_error } : {}),
  };
}
