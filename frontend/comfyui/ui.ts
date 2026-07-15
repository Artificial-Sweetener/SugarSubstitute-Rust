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
 * Own the SugarCubes host integration layer in `frontend/comfyui/ui.js`.
 */

import { app } from '/scripts/app.js';
import type { ComfyApp, ComfyExtension } from '/scripts/app.js';
import { api } from '/scripts/api.js';
import { writeWidgetValue } from './ui/graph/Markers.js';
import {
  getGroupSugarcubes,
  normalizeGroupInstanceAlias,
  setGroupSugarcubes,
} from './ui/graph/GroupMetadata.js';
import { coerceVec2, readVector2 } from './ui/graph/VectorUtils.js';
import {
  computePayloadBounds,
  drawGhostRect,
  getPlacementGroupLabel,
  readLayoutFlags,
  readLayoutStyle,
  resolvePreviewRect,
} from './ui/overlays/PlacementHelpers.js';
import {
  buildImportSummary,
  buildShiftedPlacementPayload,
  collectCubeIdsFromPayload,
  readImportPayload,
} from './ui/import/PlacementPayload.js';
import { CubeImportCommandService } from './ui/import/CubeImportCommandService.js';
import {
  applyExecutionMode,
  applyExtrasToNode,
  applyInputValueToNode,
  computeGridPosition,
  ensureInputSlot,
  resolveInputSlotIndex,
  resolveOutputSlotIndex,
  updateBoundsWithNode,
} from './ui/import/ImportNodeWriter.js';
import type {
  IdMaps,
  ImportEntryLayout,
  ImportGroupMetadata,
  ImportLayout,
  ImportLayoutGroup,
  ImportPayload,
} from './ui/import/PlacementPayload.js';
import type { ImportOptions, ImportResult } from './ui/import/CubeImportCommandService.js';
import { createPublicApi, getSugarCubesUI } from './ui/index.js';
import { computeInnerBounds } from './ui/graph/CubeBounds.js';
import { normalizeSubgraphPayload } from './ui/graph/SubgraphSerialization.js';
import { rebindSubgraphWidgetValues } from './ui/graph/WidgetSnapshots.js';
import { createHostSettingsController } from './ui/settings/HostSettingsController.js';
import type { SettingsManager } from './ui/settings/HostSettingsController.js';
import { applyCubeDefinitionIdentity } from './ui/core/CubeDefinitionKey.js';
import type { CubeFlavorPayload } from './ui/flavors/FlavorService.js';
import type { CubeGroupMetadataRecord } from './ui/graph/GroupMetadata.js';
import { isRecord } from './ui/types/common.js';
import type { UnknownRecord, Vec2 } from './ui/types/common.js';
import type {
  ComfyApplication,
  ComfyCanvas,
  ComfyGraph,
  ComfyGroup,
  ComfyNode,
  GraphId,
} from './ui/types/graph.js';

export { buildShiftedPlacementPayload };

type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

interface ExtensionManager {
  registerSidebarTab(tab: {
    id: string;
    title: string;
    tooltip: string;
    icon: string;
    type: string;
    render(container: HTMLElement): void;
    destroy(): void;
  }): void;
}

interface SugarCubesHostApp extends ComfyApplication {
  ui?: { settings?: SettingsManager };
  extensionManager?: ExtensionManager;
  registerExtension?(extension: UnknownRecord): void;
}

export interface SugarCubesExtension extends ComfyExtension, UnknownRecord {
  setup(): Promise<void>;
  beforeConfigureGraph(): void;
  afterConfigureGraph(missingNodeTypes: string[], comfyApp: ComfyApp): void;
}

interface CanvasTransform extends ComfyCanvas {
  convertCanvasToOffset?(point: Vec2): unknown;
  ds?: { scale?: number; offset?: number[] };
  last_mouse_position?: number[];
  centerOnNode?(node: ComfyNode): void;
}

interface LiveGraph extends ComfyGraph {
  add(item: ComfyNode | ComfyGroup): void;
  getNodeById?(id: GraphId): ComfyNode | null;
  createSubgraph?(payload: UnknownRecord): UnknownRecord | null;
}

interface LiveNode extends ComfyNode {
  id: GraphId;
  pos: number[];
  size: number[];
  properties: UnknownRecord;
  connect?(outputSlot: number, targetNode: ComfyNode, inputSlot: number): unknown;
  addInput?(name: string, type: string): unknown;
  configure?(value: UnknownRecord): void;
  mode?: unknown;
}

interface LiveGroup extends ComfyGroup {
  pos: number[];
  size: number[];
  properties: UnknownRecord;
  configure?(value: UnknownRecord): void;
}

interface SubgraphHint {
  fallbackName: string;
  expectedInputNames: string[];
}

interface LayoutGroupContext {
  graph: LiveGraph;
  dropOrigin: Vec2;
  bounds: ImportResult['bounds'];
  instanceAlias?: string;
  cube?: UnknownRecord;
  revision?: UnknownRecord;
}

const EXTENSION_NAME = 'SugarCubes.UI';
const IMPORT_STORAGE_KEY = 'SugarCubes.Import.LastCube';
const CUBE_INSTANCE_SCHEMA = 5;

/** Provide the authoritative UI service graph for this host extension instance. */
export const sugarCubesUI = getSugarCubesUI({
  forceNew: true,
  adapterOptions: { app, api },
  applyPreparedImport,
  reportImportOutcome,
  buildShiftedPlacementPayload,
});
const ui = sugarCubesUI;
const adapter = ui.adapter;
const storage = ui.storage;
const toastService = ui.toast;
const cubeApi = ui.api;
const overlayManager = ui.overlayManager;
const appRef = adapter.getApp() as SugarCubesHostApp | null;
const windowRef = adapter.getWindow();
const documentRef = adapter.getDocument();
const consoleRef = adapter.getConsole();
const logger = consoleRef || {
  log() {},
  warn() {},
  error() {},
  info() {},
  debug() {},
};

let registeredExtensionManager: ExtensionManager | null = null;
let sidebarRoot: HTMLDivElement | null = null;

const LAST_CUBE_STORAGE_KEYS = Object.freeze([IMPORT_STORAGE_KEY]);

function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function persistLastCubeId(value: unknown): void {
  if (value == null) {
    return;
  }
  const trimmed = typeof value === 'string' ? value.trim() : '';
  if (!trimmed) {
    return;
  }
  try {
    if (!storage) {
      return;
    }
    const seen = new Set();
    for (const key of LAST_CUBE_STORAGE_KEYS) {
      if (!key || seen.has(key)) {
        continue;
      }
      storage.writeValue(key, trimmed);
      seen.add(key);
    }
  } catch (_error) {
    // ignore storage persistence failures
  }
}

function pushToastMessage(severity: ToastSeverity, summary: string, detail: string): void {
  toastService?.push?.(severity, summary, detail);
}

function invalidateDependentCatalogs(): void {
  ui.cubeBrowser.refresh({ force: true }).catch(() => {});
}

const hostSettingsController = createHostSettingsController({
  adapter,
  appRef,
  cubeApi,
  ui,
  logger,
  pushToast: pushToastMessage,
  readErrorMessage,
  invalidateDependentCatalogs,
});

function registerSidebarTab(): void {
  const extensionManager = appRef?.extensionManager;
  if (!extensionManager?.registerSidebarTab) {
    logger.warn('SugarCubes: extension manager unavailable; sidebar tab not registered.');
    return;
  }
  if (registeredExtensionManager === extensionManager) {
    return;
  }
  sidebarRoot = null;

  extensionManager.registerSidebarTab({
    id: 'sugarcubes',
    title: 'SugarCubes',
    tooltip: 'SugarCubes',
    icon: 'mdi mdi-cube',
    type: 'custom',
    render: (container: HTMLElement) => {
      renderSidebarPanel(container);
    },
    destroy: () => {
      if (sidebarRoot && sidebarRoot.parentElement) {
        sidebarRoot.parentElement.removeChild(sidebarRoot);
      }
    },
  });
  registeredExtensionManager = extensionManager;
}

function renderSidebarPanel(container: HTMLElement | null | undefined): void {
  if (!container) {
    return;
  }
  if (!documentRef) {
    return;
  }

  if (!sidebarRoot) {
    sidebarRoot = documentRef.createElement('div');
    sidebarRoot.className = 'sugarcubes-sidebar-panel';
    Object.assign(sidebarRoot.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
      padding: '12px',
      color: 'var(--fg-color, #e8e8e8)',
      fontFamily: 'sans-serif',
    });

    const header = documentRef.createElement('div');
    header.textContent = 'SugarCubes';
    Object.assign(header.style, {
      fontSize: '14px',
      fontWeight: '600',
      letterSpacing: '0.02em',
    });
    sidebarRoot.appendChild(header);

    const tabContent = documentRef.createElement('div');
    tabContent.className = 'sugarcubes-sidebar-panel__content';
    Object.assign(tabContent.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '12px',
    });
    sidebarRoot.appendChild(tabContent);

    const librarySection = documentRef.createElement('div');
    librarySection.className = 'sugarcubes-sidebar-panel__library';
    Object.assign(librarySection.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    });

    const browserSection = documentRef.createElement('div');
    browserSection.className = 'sugarcubes-sidebar-panel__browser';
    Object.assign(browserSection.style, {
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    });
    librarySection.appendChild(browserSection);

    tabContent.appendChild(librarySection);

    ui.cubeBrowser.mountEmbedded(browserSection);
  }

  container.replaceChildren(sidebarRoot);
}

function convertCanvasPoint(canvasInstance: CanvasTransform | null, point: unknown): Vec2 | null {
  if (!canvasInstance || !Array.isArray(point)) {
    return null;
  }
  try {
    if (typeof canvasInstance.convertCanvasToOffset === 'function') {
      const converted = canvasInstance.convertCanvasToOffset(readVector2(point, 0, 0));
      if (Array.isArray(converted) && converted.length >= 2) {
        const x = Number(converted[0]);
        const y = Number(converted[1]);
        if (Number.isFinite(x) && Number.isFinite(y)) {
          return [x, y];
        }
      }
    }
    const ds = canvasInstance.ds;
    const scale = Number(ds?.scale) || 1;
    const offset = Array.isArray(ds?.offset) ? ds.offset : [0, 0];
    const x = Number(point[0] ?? 0) / scale - Number(offset[0] ?? 0);
    const y = Number(point[1] ?? 0) / scale - Number(offset[1] ?? 0);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return [x, y];
    }
  } catch (error) {
    logger.warn('SugarCubes -> convertCanvasPoint failed', error);
  }
  return null;
}

function computeDropOrigin(): Vec2 {
  const canvasInstance = adapter.getCanvas() as CanvasTransform | null;
  if (!canvasInstance) {
    return [0, 0];
  }

  const lastMouse = canvasInstance.last_mouse_position;
  if (Array.isArray(lastMouse) && Number.isFinite(lastMouse[0]) && Number.isFinite(lastMouse[1])) {
    const converted = convertCanvasPoint(canvasInstance, lastMouse);
    if (converted) {
      return converted;
    }
  }

  try {
    const canvasElement = canvasInstance.canvas ?? null;
    if (canvasElement && typeof canvasElement.getBoundingClientRect === 'function') {
      const rect = canvasElement.getBoundingClientRect();
      const relative = [rect.width / 2, rect.height / 2];
      const converted = convertCanvasPoint(canvasInstance, relative);
      if (converted) {
        return converted;
      }
    }
  } catch (_error) {
    // ignore viewport conversion issues
  }

  const ds = canvasInstance.ds ?? null;
  if (ds) {
    const offset = Array.isArray(ds.offset) ? ds.offset : [0, 0];
    const x = -Number(offset[0] ?? 0);
    const y = -Number(offset[1] ?? 0);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return [x, y];
    }
  }

  return [0, 0];
}

function reportImportOutcome(
  defaultAlias: string,
  backendWarnings: unknown[],
  importResult: Partial<ImportResult> | null,
  payloadValue: unknown,
  options: { focus?: boolean } = {},
): void {
  const payload = readImportPayload(payloadValue) ?? {};
  const warningMessages = backendWarnings.filter(
    (warning): warning is string => typeof warning === 'string' && Boolean(warning),
  );
  if (warningMessages.length) {
    pushToastMessage('warn', 'SugarCube import warnings', warningMessages.join('\n'));
  }

  const frontendWarnings = (importResult?.warnings ?? []).filter(Boolean);
  if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
    frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
  }
  if (importResult?.message && importResult.success) {
    frontendWarnings.push(importResult.message);
  }
  if (frontendWarnings.length) {
    pushToastMessage('warn', 'SugarCube import notes', frontendWarnings.join('\n'));
  }

  const summary = importResult?.summary ?? buildImportSummary(payload);
  if (!importResult?.success) {
    const detail = importResult?.message || summary;
    pushToastMessage('warn', `SugarCube ${defaultAlias} import incomplete`, detail);
    return;
  }

  pushToastMessage('success', `Imported ${defaultAlias}`, summary);
  const shouldFocus = options.focus !== false;
  if (shouldFocus) {
    const graphInstance = appRef?.graph as LiveGraph | undefined;
    if (graphInstance && importResult?.primaryNodeId != null) {
      const focusNode = graphInstance.getNodeById?.(importResult.primaryNodeId);
      const canvas = adapter.getCanvas() as CanvasTransform | null;
      if (focusNode && canvas?.centerOnNode) {
        try {
          canvas.centerOnNode(focusNode);
        } catch (_error) {
          // ignore focus failures
        }
      }
    }
  }
}

/**
 * Index legacy subgraph import hints by wrapper id from the prepared import payload.
 */
function buildImportedSubgraphHintLookup(payload: ImportPayload): Map<string, SubgraphHint> {
  const lookup = new Map<string, SubgraphHint>();
  const nodeEntries = payload.nodes ?? [];
  for (const entry of nodeEntries) {
    const classType = typeof entry?.class_type === 'string' ? entry.class_type.trim() : '';
    if (!classType) {
      continue;
    }
    const extrasMetadata = isRecord(entry.extras?._meta) ? entry.extras._meta : null;
    const title =
      (typeof entry?.layout?.title === 'string' && entry.layout.title.trim()) ||
      (typeof extrasMetadata?.title === 'string' && extrasMetadata.title.trim()) ||
      '';
    const inputs = entry?.inputs && typeof entry.inputs === 'object' ? entry.inputs : {};
    const expectedInputNames = Object.keys(inputs);
    const existing = lookup.get(classType) || { fallbackName: '', expectedInputNames: [] };
    lookup.set(classType, {
      fallbackName: title || existing.fallbackName || '',
      expectedInputNames: expectedInputNames.length
        ? expectedInputNames
        : Array.isArray(existing.expectedInputNames)
          ? existing.expectedInputNames
          : [],
    });
  }
  return lookup;
}

function registerSubgraphs(payload: ImportPayload, result: ImportResult): void {
  const subgraphs = payload.subgraphs ?? [];
  if (!subgraphs.length) {
    return;
  }
  const graph = appRef?.graph as LiveGraph | undefined;
  if (!graph?.createSubgraph) {
    result.warnings.push('Subgraph registration unavailable; skipping subgraph definitions.');
    return;
  }
  const subgraphMap = graph._subgraphs instanceof Map ? graph._subgraphs : null;
  const hintLookup = buildImportedSubgraphHintLookup(payload);
  for (const entry of subgraphs) {
    if (!entry || typeof entry !== 'object') {
      continue;
    }
    const subId = entry.id;
    if (typeof subId !== 'string' || !subId) {
      result.warnings.push('Subgraph entry missing id; skipping.');
      continue;
    }
    if (subgraphMap && subgraphMap.has(subId)) {
      continue;
    }
    try {
      const hint = hintLookup.get(subId) || { fallbackName: '', expectedInputNames: [] };
      const normalized = normalizeSubgraphPayload(entry, subId, {
        fallbackName: hint.fallbackName || '',
        expectedInputNames: Array.isArray(hint.expectedInputNames) ? hint.expectedInputNames : [],
      });
      if (!normalized) {
        result.warnings.push(`Subgraph '${subId}' could not be normalized; skipping.`);
        continue;
      }
      rebindSubgraphWidgetValues(normalized, (classType) => {
        if (!classType) return null;
        return (
          (globalThis.LiteGraph?.createNode?.(classType) as ComfyNode | null | undefined) ?? null
        );
      });
      const subgraph = graph.createSubgraph(normalized);
      if (subgraph && typeof subgraph.configure === 'function') {
        subgraph.configure(normalized);
      }
    } catch (error: unknown) {
      result.warnings.push(`Failed to register subgraph '${subId}': ${readErrorMessage(error)}`);
    }
  }
}

/**
 * Record every serialized ID that may identify a created node.
 */
function recordCreatedNodeId(
  idMap: Map<string, GraphId>,
  sourceIds: unknown[],
  createdId: GraphId | null | undefined,
): void {
  if (!idMap || createdId == null) {
    return;
  }
  for (const sourceId of sourceIds) {
    if (sourceId == null) {
      continue;
    }
    const sourceKey = String(sourceId);
    if (sourceKey) {
      idMap.set(sourceKey, createdId);
    }
  }
}

/**
 * Remap an imported metadata ID list to the actual LiteGraph IDs.
 */
function remapImportedIdList(values: unknown, idMap: Map<string, GraphId> | undefined): GraphId[] {
  if (!Array.isArray(values)) {
    return [];
  }
  const remapped: GraphId[] = [];
  for (const value of values) {
    const mapped = idMap?.get?.(String(value));
    if (mapped != null) {
      remapped.push(mapped);
    }
  }
  return remapped;
}

/**
 * Remap SugarCubes group metadata using created node and marker IDs.
 */
function remapImportedGroupMetadata(
  metadata: ImportGroupMetadata | null | undefined,
  { nodeIdMap, markerIdMap }: IdMaps = {},
): ImportGroupMetadata | null | undefined {
  if (!metadata) {
    return metadata;
  }
  const next = { ...metadata };
  if (metadata.markers && typeof metadata.markers === 'object') {
    next.markers = {
      ...metadata.markers,
      inputs: remapImportedIdList(metadata.markers.inputs, markerIdMap),
      outputs: remapImportedIdList(metadata.markers.outputs, markerIdMap),
    };
  }
  if (Array.isArray(metadata.nodes)) {
    next.nodes = remapImportedIdList(metadata.nodes, nodeIdMap);
  }
  return next;
}

/**
 * Remap imported layout group metadata before groups are created.
 */
function remapImportedLayoutIds(
  layout: ImportLayout | null,
  { nodeIdMap, markerIdMap }: IdMaps = {},
): ImportLayout | null {
  if (!layout?.groups) {
    return layout;
  }
  return {
    ...layout,
    groups: layout.groups.map((group) => {
      if (!group.sugarcubes) {
        return group;
      }
      return {
        ...group,
        sugarcubes:
          remapImportedGroupMetadata(group.sugarcubes, {
            ...(nodeIdMap ? { nodeIdMap } : {}),
            ...(markerIdMap ? { markerIdMap } : {}),
          }) ?? null,
      };
    }),
  };
}

async function applyPreparedImport(
  payloadValue: unknown,
  options: ImportOptions & { instanceAlias?: string } = {},
): Promise<ImportResult> {
  const result: ImportResult = {
    success: false,
    summary: '',
    message: '',
    warnings: [],
    missingTypes: [],
    nodesAdded: 0,
    markersAdded: 0,
    connectionsMade: 0,
    primaryNodeId: null,
    bounds: null,
  };

  const payload = readImportPayload(payloadValue);
  if (!payload) {
    result.message = 'Importer payload missing';
    return result;
  }

  const graph = appRef?.graph as LiveGraph | undefined;
  if (!graph) {
    result.message = 'Graph unavailable';
    return result;
  }

  const LiteGraphRef = adapter.getLiteGraph?.() || null;
  if (!LiteGraphRef || typeof LiteGraphRef.createNode !== 'function') {
    result.message = 'LiteGraph unavailable';
    return result;
  }

  registerSubgraphs(payload, result);

  const nodeEntries = payload.nodes ?? [];
  const markerEntries = payload.markers ?? [];
  const connectionEntries = payload.connections ?? [];

  const connectedInputs = new Map<string, Set<string>>();
  for (const connection of connectionEntries) {
    const toSymbol = typeof connection?.to?.symbol === 'string' ? connection.to.symbol : null;
    const inputName = typeof connection?.to?.input === 'string' ? connection.to.input : null;
    if (!toSymbol || !inputName) {
      continue;
    }
    const set = connectedInputs.get(toSymbol) ?? new Set();
    set.add(inputName);
    connectedInputs.set(toSymbol, set);
  }

  const existingIds = new Set<GraphId>();
  const knownNodes = Array.isArray(graph._nodes)
    ? graph._nodes
    : Array.isArray(graph.nodes)
      ? graph.nodes
      : [];
  for (const node of knownNodes) {
    if (node && node.id != null) {
      existingIds.add(node.id);
    }
  }
  const usedIds = new Set(existingIds);

  const createdNodes = new Map<string, LiveNode>();
  const createdMarkers = new Map<string, LiveNode>();
  const nodeIdMap = new Map<string, GraphId>();
  const markerIdMap = new Map<string, GraphId>();
  const bounds = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
  const hasBounds = () =>
    Number.isFinite(bounds.minX) &&
    Number.isFinite(bounds.minY) &&
    Number.isFinite(bounds.maxX) &&
    Number.isFinite(bounds.maxY);

  const dropOriginVec = readVector2(options.dropOrigin ?? payload.layout?.origin, 0, 0);

  let fallbackIndex = 0;
  const nextFallbackPosition = () => {
    const pos = computeGridPosition(dropOriginVec, fallbackIndex);
    fallbackIndex += 1;
    return pos;
  };

  const resolvePosition = (layout: ImportEntryLayout | null | undefined): Vec2 => {
    if (layout && Array.isArray(layout.pos)) {
      return readVector2(layout.pos, dropOriginVec[0], dropOriginVec[1]);
    }
    return nextFallbackPosition();
  };

  if (typeof graph.beforeChange === 'function') {
    graph.beforeChange();
  }

  try {
    for (const entry of nodeEntries) {
      const symbol = typeof entry?.symbol === 'string' ? entry.symbol : null;
      const classType = typeof entry?.class_type === 'string' ? entry.class_type : null;
      if (!symbol || !classType) {
        result.warnings.push('Node entry missing symbol or class_type; skipping.');
        continue;
      }

      const liteNode = LiteGraphRef.createNode(classType) as LiveNode | null;
      if (!liteNode) {
        result.missingTypes.push(classType);
        result.warnings.push(`Node type '${classType}' is unavailable; skipping '${symbol}'.`);
        continue;
      }

      const layout = entry?.layout || {};
      const [posX, posY] = resolvePosition(layout);
      const sizeVec = readVector2(
        layout?.size,
        Array.isArray(liteNode.size) ? liteNode.size[0] : 140,
        Array.isArray(liteNode.size) ? liteNode.size[1] : 60,
      );

      if (Array.isArray(liteNode.pos)) {
        liteNode.pos[0] = posX;
        liteNode.pos[1] = posY;
      } else {
        liteNode.pos = [posX, posY];
      }

      if (Array.isArray(liteNode.size)) {
        if (Number.isFinite(sizeVec[0])) {
          liteNode.size[0] = sizeVec[0];
        }
        if (Number.isFinite(sizeVec[1])) {
          liteNode.size[1] = sizeVec[1];
        }
      } else {
        liteNode.size = [sizeVec[0], sizeVec[1]];
      }

      if (typeof layout?.title === 'string' && layout.title) {
        liteNode.title = layout.title;
      } else {
        const metadata = isRecord(entry.extras?._meta) ? entry.extras._meta : null;
        if (typeof metadata?.title === 'string' && metadata.title) {
          liteNode.title = metadata.title;
        }
      }

      const desiredIdRaw = layout?.id ?? entry?.extras?.original_id;
      const desiredId = Number(desiredIdRaw);
      if (Number.isInteger(desiredId) && !usedIds.has(desiredId)) {
        liteNode.id = desiredId;
        usedIds.add(desiredId);
      } else {
        liteNode.id = -1;
      }

      graph.add(liteNode);
      usedIds.add(liteNode.id);
      recordCreatedNodeId(nodeIdMap, [layout?.id, entry?.extras?.original_id], liteNode.id);

      if (!liteNode.properties || typeof liteNode.properties !== 'object') {
        liteNode.properties = {};
      }
      liteNode.properties.sugarcubes_symbol = symbol;

      applyLayoutPresentation(liteNode, layout, result);
      applyExecutionMode(liteNode, entry?.mode ?? entry?.extras?.mode);

      createdNodes.set(symbol, liteNode);
      updateBoundsWithNode(bounds, liteNode);
      result.nodesAdded += 1;
      if (result.primaryNodeId == null) {
        result.primaryNodeId = liteNode.id;
      }

      const inputs = entry?.inputs && typeof entry.inputs === 'object' ? entry.inputs : {};
      const linkedInputs = connectedInputs.get(symbol);
      for (const [inputName, inputValue] of Object.entries(inputs)) {
        if (linkedInputs?.has(inputName)) {
          continue;
        }
        applyInputValueToNode(liteNode, inputName, inputValue);
      }

      const extras = entry?.extras;
      if (extras && typeof extras === 'object') {
        applyExtrasToNode(liteNode, extras);
      }
    }

    for (const entry of markerEntries) {
      const alias = typeof entry?.alias === 'string' ? entry.alias : null;
      const classType = typeof entry?.class_type === 'string' ? entry.class_type : null;
      if (!alias || !classType) {
        result.warnings.push('Marker entry missing alias or class_type; skipping.');
        continue;
      }

      const markerNode = LiteGraphRef.createNode(classType) as LiveNode | null;
      if (!markerNode) {
        result.missingTypes.push(classType);
        result.warnings.push(`Marker type '${classType}' is unavailable; skipping '${alias}'.`);
        continue;
      }

      const layout = entry?.layout || {};
      const [posX, posY] = resolvePosition(layout);
      const sizeVec = readVector2(
        layout?.size,
        Array.isArray(markerNode.size) ? markerNode.size[0] : 120,
        Array.isArray(markerNode.size) ? markerNode.size[1] : 40,
      );

      if (Array.isArray(markerNode.pos)) {
        markerNode.pos[0] = posX;
        markerNode.pos[1] = posY;
      } else {
        markerNode.pos = [posX, posY];
      }

      if (Array.isArray(markerNode.size)) {
        if (Number.isFinite(sizeVec[0])) {
          markerNode.size[0] = sizeVec[0];
        }
        if (Number.isFinite(sizeVec[1])) {
          markerNode.size[1] = sizeVec[1];
        }
      } else {
        markerNode.size = [sizeVec[0], sizeVec[1]];
      }

      if (typeof layout?.title === 'string' && layout.title) {
        markerNode.title = layout.title;
      }

      const desiredId = Number(layout?.id);
      if (Number.isInteger(desiredId) && !usedIds.has(desiredId)) {
        markerNode.id = desiredId;
        usedIds.add(desiredId);
      } else {
        markerNode.id = -1;
      }

      graph.add(markerNode);
      usedIds.add(markerNode.id);
      recordCreatedNodeId(markerIdMap, [layout?.id, entry?.id], markerNode.id);

      if (!markerNode.properties || typeof markerNode.properties !== 'object') {
        markerNode.properties = {};
      }
      markerNode.properties.sugarcubes_symbol = alias;
      const versionIdentity = readPreparedCubeIdentity(payload);
      if (versionIdentity.cubeVersion) {
        markerNode.properties.sugarcubes_cube_version = versionIdentity.cubeVersion;
      }
      if (versionIdentity.revisionRef) {
        markerNode.properties.sugarcubes_cube_revision_ref = versionIdentity.revisionRef;
      }

      applyLayoutPresentation(markerNode, layout, result);

      const widgetValues =
        entry?.widget_values && typeof entry.widget_values === 'object' ? entry.widget_values : {};
      for (const [widgetName, widgetValue] of Object.entries(widgetValues)) {
        writeWidgetValue(markerNode, widgetName, widgetValue);
      }

      createdMarkers.set(alias, markerNode);
      updateBoundsWithNode(bounds, markerNode);
      result.markersAdded += 1;
      if (result.primaryNodeId == null) {
        result.primaryNodeId = markerNode.id;
      }
    }

    const resolveCreatedNode = (symbol: string): LiveNode | undefined =>
      createdNodes.get(symbol) ?? createdMarkers.get(symbol);

    for (const connection of connectionEntries) {
      const fromSymbol =
        typeof connection?.from?.symbol === 'string' ? connection.from.symbol : null;
      const toSymbol = typeof connection?.to?.symbol === 'string' ? connection.to.symbol : null;
      const inputName = typeof connection?.to?.input === 'string' ? connection.to.input : null;
      if (!fromSymbol || !toSymbol || !inputName) {
        continue;
      }

      const fromNode = resolveCreatedNode(fromSymbol);
      const toNode = resolveCreatedNode(toSymbol);
      if (!fromNode || !toNode) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (node missing).`,
        );
        continue;
      }

      if (!Array.isArray(fromNode.outputs) || fromNode.outputs.length === 0) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (no outputs).`,
        );
        continue;
      }

      let slotIndex = resolveOutputSlotIndex(fromNode, connection?.from?.slot);
      if (!Number.isInteger(slotIndex)) {
        slotIndex = 0;
      }
      if (slotIndex < 0) {
        slotIndex = 0;
      }
      if (slotIndex >= fromNode.outputs.length) {
        slotIndex = fromNode.outputs.length - 1;
      }

      if (!ensureInputSlot(toNode, inputName)) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (input unavailable).`,
        );
        continue;
      }

      const inputIndex = resolveInputSlotIndex(toNode, inputName);
      if (inputIndex === -1) {
        result.warnings.push(
          `Skipping connection '${fromSymbol}' -> '${toSymbol}.${inputName}' (input unresolved).`,
        );
        continue;
      }

      try {
        if (!fromNode.connect) {
          throw new Error(`Node '${fromSymbol}' cannot create connections.`);
        }
        fromNode.connect(slotIndex, toNode, inputIndex);
        result.connectionsMade += 1;
      } catch (error: unknown) {
        const message = readErrorMessage(error);
        result.warnings.push(
          `Failed to connect '${fromSymbol}' -> '${toSymbol}.${inputName}': ${message}`,
        );
      }
    }
  } catch (error: unknown) {
    const message = readErrorMessage(error);
    result.message = message;
    result.warnings.push(`Importer error: ${message}`);
  } finally {
    if (typeof graph.afterChange === 'function') {
      graph.afterChange();
    }
  }

  if (graph.setDirtyCanvas) {
    graph.setDirtyCanvas(true, true);
  }
  const canvas = adapter.getCanvas();
  if (canvas?.setDirty) {
    canvas.setDirty(true, true);
  }

  if (hasBounds()) {
    result.bounds = bounds;
  }

  const remappedLayout = remapImportedLayoutIds(payload.layout ?? null, { nodeIdMap, markerIdMap });
  recreateLayoutGroups(remappedLayout, {
    ...(options.instanceAlias ? { instanceAlias: options.instanceAlias } : {}),
    dropOrigin: dropOriginVec,
    graph,
    bounds: result.bounds,
    ...(payload.cube ? { cube: payload.cube } : {}),
    ...(isRecord(payload.revision) ? { revision: payload.revision } : {}),
  });
  result.summary = `nodes ${result.nodesAdded}, markers ${result.markersAdded}, links ${result.connectionsMade}`;
  result.success = result.nodesAdded + result.markersAdded > 0;
  ui.instanceManager.refresh({ graph, reason: 'import', force: true });
  ui.instanceManager.scheduleRefresh({ graph, reason: 'import', force: true });
  if (result.success) {
    const cubeIds = Array.from(collectCubeIdsFromPayload(payload));
    if (cubeIds.length) {
      ui.dirtyManager.markLocalBaseline({ graph, cubeIds });
    }
  }
  ui.dirtyManager.requestRefresh({ graph, reason: 'import' });

  if (result.missingTypes.length) {
    result.missingTypes = Array.from(new Set(result.missingTypes));
  }

  if (!result.success && !result.message) {
    result.message = 'No nodes were created';
  }

  return result;
}

function recreateLayoutGroups(layout: ImportLayout | null, context: LayoutGroupContext): void {
  if (!layout) {
    return;
  }
  const graph = context.graph;

  const dropOrigin = readVector2(
    Array.isArray(context.dropOrigin) ? context.dropOrigin : [0, 0],
    0,
    0,
  );
  const baseOrigin = readVector2(layout.origin, dropOrigin[0], dropOrigin[1]);
  const groups = layout.groups ?? [];

  const createGroup = (
    groupPayload: ImportLayoutGroup,
    options: { title?: string } = {},
  ): LiveGroup | null => {
    const sugarcubesPayload = groupPayload.sugarcubes ?? null;
    const cubeMetadata = isRecord(context.cube?.metadata) ? context.cube.metadata : {};
    const cubeIcon =
      (isRecord(context.cube?.icon) && context.cube.icon) ||
      (isRecord(cubeMetadata.icon) && cubeMetadata.icon) ||
      null;
    const canonicalDefaultAlias =
      (typeof context.cube?.default_alias === 'string' && context.cube.default_alias.trim()) ||
      (typeof cubeMetadata.default_alias === 'string' && cubeMetadata.default_alias.trim()) ||
      '';
    const instanceAliasSeed =
      (typeof context.instanceAlias === 'string' && context.instanceAlias.trim()) ||
      canonicalDefaultAlias ||
      (typeof sugarcubesPayload?.default_alias === 'string' &&
        sugarcubesPayload.default_alias.trim()) ||
      (typeof context.cube?.cube_id === 'string' && context.cube.cube_id.trim()) ||
      '';
    const title =
      typeof options.title === 'string' && options.title
        ? options.title
        : instanceAliasSeed
          ? instanceAliasSeed
          : 'SugarCube';
    const liteGraph = adapter.getLiteGraph?.() || null;
    if (!liteGraph?.LGraphGroup) {
      return null;
    }
    const group = new liteGraph.LGraphGroup(title) as LiveGroup;
    graph.add(group);

    const bounding = Array.isArray(groupPayload?.bounding) ? groupPayload.bounding : null;
    if (bounding && bounding.length === 4) {
      const [bx = 0, by = 0, bw = 0, bh = 0] = bounding.map((value) => Number(value) || 0);
      group.pos = [baseOrigin[0] + bx, baseOrigin[1] + by];
      group.size = [bw, bh];
    } else if (context.bounds) {
      const { minX, minY, maxX, maxY } = context.bounds;
      group.pos = [minX, minY];
      group.size = [maxX - minX, maxY - minY];
    } else {
      group.pos = [baseOrigin[0], baseOrigin[1]];
      group.size = [640, 480];
    }

    if (typeof groupPayload.color === 'string' && groupPayload.color) {
      group.color = groupPayload.color;
    }
    if (typeof groupPayload.bgcolor === 'string' && groupPayload.bgcolor) {
      group.bgcolor = groupPayload.bgcolor;
    }
    if (typeof groupPayload.font_size === 'number' && groupPayload.font_size) {
      group.font_size = groupPayload.font_size;
    }
    if (isRecord(groupPayload.flags)) {
      group.flags = { ...(isRecord(group.flags) ? group.flags : {}), ...groupPayload.flags };
    }

    if (sugarcubesPayload) {
      const next: CubeGroupMetadataRecord = applyCubeDefinitionIdentity(
        { ...sugarcubesPayload },
        {
          cubeId: context.cube?.cube_id,
          version: context.cube?.version,
          revisionRef: context.revision?.revision_ref,
        },
      );
      if (context.cube?.target_model) {
        next.target_model = context.cube.target_model;
      }
      if (canonicalDefaultAlias) {
        next.default_alias = canonicalDefaultAlias;
      }
      if (cubeIcon) {
        next.icon = cubeIcon;
      }
      const cubeFlavorPayload: CubeFlavorPayload | null = context.cube
        ? {
            ...context.cube,
            ...(isRecord(context.cube.flavors)
              ? { flavors: { authored: context.cube.flavors.authored } }
              : { flavors: {} }),
          }
        : null;
      const flavorMetadata = cubeFlavorPayload
        ? ui.flavorService.buildImportedMetadata(cubeFlavorPayload)
        : null;
      if (flavorMetadata) {
        Object.assign(next, flavorMetadata);
      }
      if (!next.schema) {
        next.schema = CUBE_INSTANCE_SCHEMA;
      }
      if (instanceAliasSeed) {
        next.instance_alias = instanceAliasSeed;
      } else {
        delete next.instance_alias;
      }
      const normalized = normalizeGroupInstanceAlias(group, next, instanceAliasSeed);
      if (normalized?.metadata) {
        setGroupSugarcubes(group, normalized.metadata);
        Object.defineProperty(group, '__sugarcubes_imported', {
          value: true,
          writable: true,
          configurable: true,
          enumerable: false,
        });
      }
    }

    return group;
  };

  if (groups.length) {
    for (const groupPayload of groups) {
      createGroup(groupPayload);
    }
    return;
  }

  const bounds = context.bounds || null;
  const layoutCube = isRecord(layout.cube) ? layout.cube : {};
  const contextMetadata = isRecord(context.cube?.metadata) ? context.cube.metadata : {};
  const syntheticPayload: ImportLayoutGroup = {
    title:
      context.instanceAlias ||
      (typeof layoutCube.name === 'string' ? layoutCube.name : '') ||
      'SugarCube',
    bounding: bounds
      ? [bounds.minX, bounds.minY, bounds.maxX - bounds.minX, bounds.maxY - bounds.minY]
      : null,
    color: '#3f789e',
    bgcolor: '#3f5159',
    sugarcubes: {
      managed: true,
      cube_id: typeof context.cube?.cube_id === 'string' ? context.cube.cube_id : '',
      default_alias:
        (typeof contextMetadata.default_alias === 'string' &&
          contextMetadata.default_alias.trim()) ||
        (typeof context.cube?.default_alias === 'string' ? context.cube.default_alias : '') ||
        context.instanceAlias ||
        (typeof context.cube?.cube_id === 'string' ? context.cube.cube_id : '') ||
        '',
      target_model: typeof context.cube?.target_model === 'string' ? context.cube.target_model : '',
    },
  };
  createGroup(syntheticPayload, {
    title: typeof syntheticPayload.title === 'string' ? syntheticPayload.title : 'SugarCube',
  });
}

function readPreparedCubeIdentity(payload: ImportPayload): {
  cubeId: string;
  cubeVersion: string;
  revisionRef: string;
} {
  const cube = payload.cube ?? {};
  const revision = isRecord(payload.revision) ? payload.revision : {};
  return {
    cubeId: typeof cube.cube_id === 'string' ? cube.cube_id.trim() : '',
    cubeVersion: typeof cube.version === 'string' ? cube.version.trim() : '',
    revisionRef:
      typeof revision.revision_ref === 'string' && revision.revision_ref.trim()
        ? revision.revision_ref.trim()
        : 'WORKTREE',
  };
}

function applyLayoutPresentation(
  node: LiveNode,
  layout: ImportEntryLayout | null | undefined,
  result: ImportResult,
): void {
  if (!node || !layout) {
    return;
  }
  const flags = readLayoutFlags(layout);
  if (flags && flags.collapsed === true && typeof node.collapse === 'function') {
    const alreadyCollapsed = Boolean(isRecord(node.flags) && node.flags.collapsed);
    if (!alreadyCollapsed) {
      try {
        node.collapse(true);
      } catch (error: unknown) {
        const message = readErrorMessage(error);
        const label = node.title || node.name || node.type || `node-${node.id ?? '?'}`;
        result.warnings.push(`Failed to collapse '${label}': ${message}`);
      }
    }
  }

  const style = readLayoutStyle(layout);
  if (style) {
    if (typeof style.color === 'string' && style.color) {
      node.color = style.color;
    }
    if (typeof style.bgcolor === 'string' && style.bgcolor) {
      node.bgcolor = style.bgcolor;
    }
    if (style.shape !== undefined) {
      node.shape = style.shape;
    }
  }
}

function focusImportedNode(result: ImportResult): void {
  if (result.primaryNodeId == null) return;
  const graph = appRef?.graph as LiveGraph | undefined;
  const node = graph?.getNodeById?.(result.primaryNodeId);
  const canvas = adapter.getCanvas() as CanvasTransform | null;
  if (!node || !canvas?.centerOnNode) return;
  try {
    canvas.centerOnNode(node);
  } catch (_error) {
    // Ignore host focus failures after a successful import.
  }
}

const importCommandService = new CubeImportCommandService({
  api: cubeApi,
  applyPreparedImport,
  computeDropOrigin,
  focusImportedNode,
  persistLastCubeId: (cubeId) => persistLastCubeId(cubeId),
  pushToast: pushToastMessage,
  readErrorMessage,
});
ui.cubeBrowser.configure({
  actions: {
    computeDropOrigin,
    importCubeByName: (cubeId, options) => importCommandService.importCurrent(cubeId, options),
    importCubeRevision: (cubeId, revisionRef, options) =>
      importCommandService.importRevision(cubeId, revisionRef, options),
    onCubesUpdated: (cubes) => ui.dirtyManager.updateKnownCubes(cubes),
    openConfirmDialog: (options) => ui.confirmDialog.open(options),
    promoteCube: (cube) => ui.promotionService.promote(cube),
    reconcileCubeIdentity: (identity) => ui.identityReconciler.reconcile(identity),
    startCubePlacement: (cubeId, options) => overlayManager.placement.start(cubeId, options),
  },
  helpers: {
    coerceVec2,
    computePayloadBounds: (entries, ctx) =>
      computePayloadBounds(entries, ctx, adapter.getLiteGraph?.()),
    drawGhostRect,
    getPlacementGroupLabel: (defaultAlias, group) =>
      getPlacementGroupLabel(defaultAlias, group, getGroupSugarcubes),
    readVector2,
    resolvePreviewRect: (entry, pos, size, ctx) =>
      resolvePreviewRect(entry, pos, size, ctx, adapter.getLiteGraph?.()),
  },
  placement: {
    commit: () => overlayManager.placement.commit(),
    computeOriginFromEvent: (event) => overlayManager.placement.computeOriginFromEvent(event),
    getState: () => overlayManager.placement.getState(),
    isPointerOverCanvas: (event) => overlayManager.placement.isPointerOverCanvas(event),
    setCommitInProgress: (value) => overlayManager.placement.setCommitInProgress(value),
    setDirty: () => overlayManager.placement.setDirty(),
    setOrigin: (origin) => overlayManager.placement.setOrigin(origin),
    start: (cubeId, options) =>
      overlayManager.placement.start(cubeId, {
        closeBrowser: options.closeBrowser,
        ...(options.defaultAlias ? { defaultAlias: options.defaultAlias } : {}),
      }),
    stop: (reason) => overlayManager.placement.stop(reason),
  },
});

/** Define the ComfyUI extension lifecycle owned by SugarCubes. */
export const sugarCubesExtension: SugarCubesExtension = {
  name: EXTENSION_NAME,
  async setup() {
    try {
      registerSidebarTab();
      hostSettingsController.register();
      await ui.setup();
      await hostSettingsController.refresh({ checkForUpdates: false });
      const graph = appRef?.graph;
      overlayManager.proximity.refreshOverlayState({
        recompute: true,
        ...(graph ? { graph } : {}),
      });
      hostSettingsController.refreshUi();
      ui.instanceManager.scheduleRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
      ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
    } catch (error: unknown) {
      logger.error('SugarCubes: setup failed', error);
      throw error;
    }
  },
  beforeConfigureGraph() {
    overlayManager.proximity.resetOverlayState();
  },
  afterConfigureGraph(_missingNodeTypes: string[], comfyApp) {
    const graph = comfyApp.graph ?? appRef?.graph;
    overlayManager.proximity.refreshOverlayState({
      recompute: true,
      ...(graph ? { graph } : {}),
    });
    ui.instanceManager.scheduleRefresh({ ...(graph ? { graph } : {}), reason: 'configure' });
    ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'configure' });
  },
};

app.registerExtension(sugarCubesExtension);

const debugApi = {
  getDirtyState(instanceId: unknown) {
    return ui.dirtyManager.getDebugState(instanceId);
  },
  bounds: {
    get(instanceId: unknown) {
      if (!instanceId) {
        return null;
      }
      const graph = appRef?.graph || null;
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      const entry = index?.instanceById?.get?.(String(instanceId)) || null;
      if (!entry?.metadata?.bounds) {
        return null;
      }
      return {
        bounds: entry.metadata.bounds,
        inner: computeInnerBounds(entry.metadata.bounds),
      };
    },
    reconcile(instanceId: unknown) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.boundsReconciler) {
        return { changed: [] };
      }
      const result = ui.boundsReconciler.reconcileAll({ graph });
      const changed = Array.from(result.changed || []);
      if (instanceId && !changed.includes(String(instanceId))) {
        return { changed: [] };
      }
      return { changed };
    },
    resolveCollisions(instanceId: unknown) {
      const graph = appRef?.graph || null;
      if (!graph || !ui.collisionService || !instanceId) {
        return { moved: false };
      }
      const index = ui.containmentService?.buildIndex?.(graph) || null;
      return ui.collisionService.resolveCollisions({
        graph,
        activeInstanceId: String(instanceId),
        index,
      });
    },
  },
  layout: {
    service: ui.layoutService,
    appendCube: (options: Parameters<typeof ui.layoutService.appendCube>[0]) =>
      ui.layoutService.appendCube(options),
    insertBetween: (options: Parameters<typeof ui.layoutService.insertBetween>[0]) =>
      ui.layoutService.insertBetween(options),
    insertBefore: (options: Parameters<typeof ui.layoutService.insertBefore>[0]) =>
      ui.layoutService.insertBefore(options),
    swapOrder: (options: Parameters<typeof ui.layoutService.swapOrder>[0]) =>
      ui.layoutService.swapOrder(options),
    replaceCube: (options: Parameters<typeof ui.layoutService.replaceCube>[0]) =>
      ui.layoutService.replaceCube(options),
  },
};

if (windowRef) {
  Object.assign(windowRef, {
    SugarCubes: createPublicApi(ui),
    SugarCubesDebug: debugApi,
  });
}
