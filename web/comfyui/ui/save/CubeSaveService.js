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
 * Coordinate implementation saves and personal forks for SugarCubes.
 */

import { isCanonicalCubeId } from '../core/CubeId.js';
import {
  defaultSupportedModelsForTarget,
  deriveRouteFromCubeId,
  deriveTargetModelFromCubeId,
  normalizeSupportedModels,
  normalizeTargetModel,
} from '../core/ModelTargets.js';
import {
  isCubeMarkerType,
  updateMarkersForCubeId,
  updateMarkersForIds,
} from '../graph/CubeMarkers.js';
import { InstanceBuilder } from '../graph/InstanceBuilder.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { getGroupSugarcubes, setGroupSugarcubes } from '../graph/GroupMetadata.js';
import { readWidgetValue } from '../graph/Markers.js';
import { enrichWorkflowPayload } from '../graph/WorkflowPayloadBuilder.js';
import {
  CURRENT_REVISION_REF,
  buildCubeDefinitionKey,
  isCurrentRevisionRef,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import { suggestPersonalCubeIdentity } from '../create/PersonalCubeIdentity.js';
import {
  SugarCubeExportError,
  buildErrorDetail,
  formatSaveSummaryEntry,
  formatViolations,
} from './SaveFeedback.js';

const STALE_SAVE_MODE_LATEST = 'latest';

/**
 * Coordinate durable cube saves, personal identity assignment, and save-time forks.
 */
export class CubeSaveService {
  constructor({
    adapter,
    api,
    toast,
    instanceManager,
    dirtyManager,
    cubeBrowser,
    versionDialog,
    dialogs,
    saveReconciler,
  } = {}) {
    this.adapter = adapter;
    this.api = api;
    this.toast = toast;
    this.instanceManager = instanceManager;
    this.dirtyManager = dirtyManager;
    this.cubeBrowser = cubeBrowser;
    this.versionDialog = versionDialog;
    this.dialogs = dialogs || null;
    this.saveReconciler = saveReconciler || null;
  }

  async save({ cubeIds = null, button = null } = {}) {
    return this.saveImplementation({ cubeIds, button });
  }

  async saveImplementation({ cubeIds = null, button = null } = {}) {
    const setBusy =
      typeof button?.enabled === 'boolean'
        ? (busy) => {
            button.enabled = !busy;
            button.element?.classList?.toggle('sugarcubes-save--busy', Boolean(busy));
          }
        : (_busy) => {};

    setBusy(true);
    try {
      const appRef = this.adapter?.getApp?.();
      const graph = appRef?.graph;
      if (!graph) {
        throw new Error('Unable to access the current graph');
      }

      const { assigned, replacements } = await this.ensureCubeIds(graph);
      const assignedIds = assigned.map((entry) => entry.cubeId);
      const targetCubeIds = this.resolveCubeIds(cubeIds);
      const replacementIds = Array.from(replacements.values());
      const cubeEntries = Array.isArray(this.cubeBrowser?.getCubes?.())
        ? this.cubeBrowser.getCubes()
        : [];
      const cubeIndex = this.buildCubeEntryIndex(cubeEntries);
      const sourceMetadataIndex = this.buildInstanceSaveMetadataIndex(graph);
      const usedNames = new Set(
        cubeEntries
          .map((entry) => (typeof entry?.name === 'string' ? entry.name : ''))
          .filter(Boolean),
      );

      const savePlan = [];
      const previousIdByCubeId = new Map();
      for (const [previousCubeId, nextCubeId] of replacements.entries()) {
        if (typeof previousCubeId === 'string' && previousCubeId.trim() && nextCubeId) {
          previousIdByCubeId.set(nextCubeId, previousCubeId.trim());
        }
      }
      const finalCubeIds = Array.from(
        new Set([
          ...targetCubeIds.filter(
            (cubeId) => isCanonicalCubeId(cubeId) && !replacements.has(cubeId),
          ),
          ...assignedIds,
          ...replacementIds,
        ]),
      );
      if (!finalCubeIds.length) {
        this.pushToastMessage('info', 'No changes', 'No dirty SugarCubes to save.');
        return;
      }
      for (const cubeId of finalCubeIds) {
        const entry = cubeIndex.get(cubeId);
        const sourceMetadata = this.resolveSavePlanSourceMetadata(cubeId, sourceMetadataIndex);
        const entryMetadata = this.buildSaveEntryMetadata({
          cubeId,
          browserEntry: entry,
          sourceMetadata,
        });
        if (!entry) {
          savePlan.push({
            cubeId,
            forked: false,
            lineage: null,
            metadata: entryMetadata,
            previousCubeId: previousIdByCubeId.get(cubeId) || '',
            latestVersion: '',
            ...sourceMetadata,
          });
          continue;
        }
        if (this.isWritableEntry(entry)) {
          savePlan.push({
            cubeId,
            forked: false,
            lineage: null,
            metadata: entryMetadata,
            previousCubeId: previousIdByCubeId.get(cubeId) || '',
            latestVersion: normalizeCubeVersion(entry.version),
            ...sourceMetadata,
          });
          continue;
        }
        const forkedName = this.buildForkedName(entry.name || 'SugarCube', usedNames);
        usedNames.add(forkedName);
        const reservedIds = [...cubeIndex.keys(), ...savePlan.map((candidate) => candidate.cubeId)];
        const forkedId = suggestPersonalCubeIdentity(forkedName, reservedIds).cubeId;
        const updatedMarkers = updateMarkersForCubeId(graph, cubeId, {
          cubeId: forkedId,
          defaultAlias: forkedName,
        });
        if (!updatedMarkers) {
          throw new Error(`Unable to fork cube '${entry.name || cubeId}' (markers missing).`);
        }
        savePlan.push({
          cubeId: forkedId,
          forked: true,
          lineage: this.buildLineagePayload(entry),
          metadata: this.buildSaveEntryMetadata({
            cubeId: forkedId,
            browserEntry: { ...entry, target_model: '', supported_models: [] },
          }),
          previousCubeId: cubeId,
          staleRevision: false,
          sourceEntries: [],
          sourceRevisionRef: '',
          sourceVersion: '',
          sourceDefinitionKey: '',
          staleSaveMode: '',
          latestVersion: normalizeCubeVersion(entry.version),
        });
      }

      const historicalChoice = await this.resolveHistoricalSaveChoice(savePlan);
      if (historicalChoice === null) {
        return;
      }
      if (historicalChoice === 'fork') {
        await this.forkStaleSaveEntries({ graph, savePlan, cubeIndex, usedNames });
      } else if (historicalChoice === STALE_SAVE_MODE_LATEST) {
        for (const entry of savePlan) {
          if (entry.staleRevision) {
            entry.staleSaveMode = STALE_SAVE_MODE_LATEST;
          }
        }
      }

      if (savePlan.some((entry) => entry.forked)) {
        this.instanceManager?.scheduleRefresh?.({ graph, reason: 'fork' });
        this.dirtyManager?.scheduleRefresh?.({ graph, reason: 'fork' });
      }

      const promptResult = appRef?.graphToPrompt?.();
      const resolved =
        promptResult && typeof promptResult.then === 'function' ? await promptResult : promptResult;
      const graphPayload = resolved?.output ?? resolved?.prompt ?? resolved;
      if (!graphPayload || typeof graphPayload !== 'object') {
        throw new Error('Unable to serialize the current graph');
      }

      const workflowPayload = resolved?.workflow ?? null;
      const workflowSupported =
        workflowPayload && typeof workflowPayload === 'object' && !Array.isArray(workflowPayload);
      if (!workflowSupported) {
        throw new Error('Workflow payload unavailable');
      }
      const enrichedWorkflowPayload = enrichWorkflowPayload(workflowPayload, graph);
      const requestBody = {
        graph: graphPayload,
        cubes: savePlan.map((entry) => ({
          cube_id: entry.cubeId,
          forked: entry.forked,
          lineage: entry.lineage,
          previous_cube_id: entry.previousCubeId,
          source_revision_ref: entry.sourceRevisionRef || '',
          source_version: entry.sourceVersion || '',
          source_definition_key: entry.sourceDefinitionKey || '',
          stale_save_mode: entry.staleSaveMode || '',
          ...(entry.metadata ? { metadata: entry.metadata } : {}),
        })),
        workflow: enrichedWorkflowPayload,
        workflow_version: enrichedWorkflowPayload?.version ?? null,
      };

      const { response, data } = await this.api.saveImplementation(JSON.stringify(requestBody), {
        headers: { 'Content-Type': 'application/json' },
      });
      const errorPayload = data?.error;
      if (!response.ok || errorPayload) {
        const message = errorPayload?.message ?? response.statusText ?? 'Export failed';
        const detail = buildErrorDetail(errorPayload);
        throw new SugarCubeExportError(message, detail, errorPayload?.violations);
      }

      const saved = Array.isArray(data?.saved) ? data.saved : [];
      const summary = saved.length
        ? saved.map((entry) => formatSaveSummaryEntry(entry)).join('\n')
        : 'No cubes were saved';
      this.pushToastMessage('success', 'SugarCubes exported', summary);
      this.cubeBrowser?.refresh?.({ force: true }).catch((_error) => {});

      const warnings = Array.isArray(data?.warnings) ? data.warnings.filter(Boolean) : [];
      if (warnings.length) {
        this.pushToastMessage('warn', 'SugarCubes warnings', warnings.join('\n'));
      }

      const versionSuggestions = Array.isArray(data?.version_suggestions)
        ? data.version_suggestions
        : [];
      if (versionSuggestions.length) {
        await this.versionDialog?.open?.(versionSuggestions);
      }

      const savedIds = saved
        .map((entry) => (typeof entry?.cube_id === 'string' ? entry.cube_id : ''))
        .filter(Boolean);
      if (savedIds.length) {
        if (!this.saveReconciler?.reconcile) {
          throw new Error('Cube save reconciler is unavailable');
        }
        await this.saveReconciler.reconcile({
          graph,
          saved,
          fallbackCubeIds: savedIds,
          markerIdsByCubeId: this.buildSaveReconciliationTargets(savePlan),
          reason: 'save',
        });
      }
    } catch (error) {
      const exportError =
        error instanceof SugarCubeExportError ? error : SugarCubeExportError.from(error);
      const detail = exportError.detail || formatViolations(exportError.violations);
      this.pushToastMessage('error', exportError.message, detail);
      this.adapter?.getConsole?.()?.error?.(exportError.message);
    } finally {
      setBusy(false);
    }
  }

  buildInstanceSaveMetadataIndex(graph) {
    const byCubeId = new Map();
    const addEntry = (entry) => {
      const cubeId = typeof entry?.cubeId === 'string' ? entry.cubeId.trim() : '';
      if (!cubeId) {
        return;
      }
      const list = byCubeId.get(cubeId) || [];
      const dedupeKey = `${entry.instanceId || ''}|${entry.sourceDefinitionKey || ''}|${entry.markerIds?.join(',') || ''}`;
      if (
        dedupeKey.trim() &&
        list.some(
          (existing) =>
            `${existing.instanceId || ''}|${existing.sourceDefinitionKey || ''}|${existing.markerIds?.join(',') || ''}` ===
            dedupeKey,
        )
      ) {
        return;
      }
      list.push(entry);
      byCubeId.set(cubeId, list);
    };

    for (const group of getGraphGroups(graph)) {
      const metadata = getGroupSugarcubes(group);
      if (!metadata?.managed) {
        continue;
      }
      const cubeId = typeof metadata.cube_id === 'string' ? metadata.cube_id.trim() : '';
      if (!cubeId) {
        continue;
      }
      const sourceVersion = normalizeCubeVersion(metadata.cube_version);
      const sourceRevisionRef = normalizeRevisionRef(metadata.cube_revision_ref);
      const sourceDefinitionKey =
        typeof metadata.cube_definition_key === 'string' && metadata.cube_definition_key.trim()
          ? metadata.cube_definition_key.trim()
          : buildCubeDefinitionKey(cubeId, sourceVersion);
      addEntry({
        cubeId,
        defaultAlias:
          typeof metadata.default_alias === 'string' && metadata.default_alias.trim()
            ? metadata.default_alias.trim()
            : cubeId,
        sourceVersion,
        sourceRevisionRef,
        sourceDefinitionKey,
        staleRevision: !isCurrentRevisionRef(sourceRevisionRef),
        targetModel:
          typeof metadata.target_model === 'string' && metadata.target_model.trim()
            ? metadata.target_model.trim()
            : '',
        supportedModels: Array.isArray(metadata.supported_models)
          ? metadata.supported_models.filter((value) => typeof value === 'string')
          : [],
        markerIds: this.extractMetadataMarkerIds(metadata),
        group,
        instanceId:
          typeof metadata.instance_id === 'string' && metadata.instance_id.trim()
            ? metadata.instance_id.trim()
            : '',
      });
    }

    const builder =
      this.instanceManager?.instanceBuilder ||
      new InstanceBuilder({ logger: this.adapter?.getConsole?.() });
    for (const instance of builder.build(graph)) {
      if (!instance?.cubeId) {
        continue;
      }
      const sourceRevisionRef = normalizeRevisionRef(instance.cubeRevisionRef);
      const sourceVersion = normalizeCubeVersion(instance.cubeVersion);
      addEntry({
        cubeId: instance.cubeId,
        defaultAlias: instance.defaultAlias || instance.cubeId,
        sourceVersion,
        sourceRevisionRef,
        sourceDefinitionKey:
          instance.cubeDefinitionKey || buildCubeDefinitionKey(instance.cubeId, sourceVersion),
        staleRevision: !isCurrentRevisionRef(sourceRevisionRef),
        targetModel: instance.targetModel || '',
        supportedModels: Array.isArray(instance.supportedModels) ? instance.supportedModels : [],
        markerIds: Array.isArray(instance.markerIds) ? instance.markerIds : [],
        group: null,
        instanceId: instance.instanceId || '',
      });
    }
    return byCubeId;
  }

  extractMetadataMarkerIds(metadata) {
    const rawMarkers = metadata?.markers;
    if (Array.isArray(rawMarkers)) {
      return rawMarkers.filter((value) => value != null);
    }
    if (!rawMarkers || typeof rawMarkers !== 'object') {
      return [];
    }
    return Object.values(rawMarkers).flatMap((value) =>
      Array.isArray(value) ? value.filter((entry) => entry != null) : [],
    );
  }

  resolveSavePlanSourceMetadata(cubeId, sourceMetadataIndex) {
    const entries = Array.isArray(sourceMetadataIndex?.get?.(cubeId))
      ? sourceMetadataIndex.get(cubeId)
      : [];
    const staleEntries = entries.filter((entry) => entry.staleRevision);
    const selected = staleEntries[0] || entries[0] || null;
    return {
      sourceEntries: entries,
      sourceRevisionRef: selected?.sourceRevisionRef || '',
      sourceVersion: selected?.sourceVersion || '',
      sourceDefinitionKey: selected?.sourceDefinitionKey || '',
      staleRevision: Boolean(staleEntries.length),
      staleSaveMode: '',
      selectedSourceEntry: selected,
      defaultAlias: selected?.defaultAlias || '',
      targetModel: selected?.targetModel || '',
      supportedModels: selected?.supportedModels || [],
    };
  }

  buildSaveEntryMetadata({ cubeId, browserEntry = null, sourceMetadata = null } = {}) {
    let defaultAlias = '';
    try {
      defaultAlias = deriveRouteFromCubeId(cubeId);
    } catch (_error) {
      defaultAlias = '';
    }
    const targetModel =
      normalizeTargetModel(browserEntry?.target_model) ||
      normalizeTargetModel(sourceMetadata?.targetModel) ||
      this.deriveTargetModelFromCubeIdSafe(cubeId);
    if (!defaultAlias && !targetModel) {
      return null;
    }
    const supportedSource = Array.isArray(browserEntry?.supported_models)
      ? browserEntry.supported_models
      : Array.isArray(sourceMetadata?.supportedModels)
        ? sourceMetadata.supportedModels
        : defaultSupportedModelsForTarget(targetModel);
    return {
      ...(defaultAlias ? { default_alias: defaultAlias } : {}),
      target_model: targetModel,
      supported_models: normalizeSupportedModels(supportedSource, { targetModel }),
    };
  }

  deriveTargetModelFromCubeIdSafe(cubeId) {
    try {
      return normalizeTargetModel(deriveTargetModelFromCubeId(cubeId));
    } catch (_error) {
      return '';
    }
  }

  async resolveHistoricalSaveChoice(savePlan) {
    const staleEntries = savePlan.filter((entry) => entry.staleRevision);
    if (!staleEntries.length) {
      return '';
    }
    const choice = await this.dialogs?.chooseHistoricalVersionSaveAction?.({
      entries: staleEntries.map((entry) => ({
        cubeId: entry.cubeId,
        defaultAlias: entry.defaultAlias || entry.cubeId,
        sourceVersion: entry.sourceVersion,
        sourceRevisionRef: entry.sourceRevisionRef,
      })),
    });
    if (choice === STALE_SAVE_MODE_LATEST || choice === 'fork') {
      return choice;
    }
    return null;
  }

  async forkStaleSaveEntries({ graph, savePlan, cubeIndex, usedNames }) {
    for (const entry of savePlan) {
      if (!entry.staleRevision) {
        continue;
      }
      const sourceEntries = Array.isArray(entry.sourceEntries)
        ? entry.sourceEntries.filter((source) => source.staleRevision)
        : [];
      const markerIds = Array.from(
        new Set(sourceEntries.flatMap((source) => source.markerIds || []).filter(Boolean)),
      );
      if (!markerIds.length) {
        throw new Error(
          `Unable to fork '${entry.defaultAlias || entry.cubeId}' (markers missing).`,
        );
      }
      const browserEntry = cubeIndex.get(entry.cubeId) || {};
      const forkedName = this.buildForkedName(
        entry.defaultAlias || browserEntry.name || 'SugarCube',
        usedNames,
      );
      usedNames.add(forkedName);
      const reservedIds = [...cubeIndex.keys(), ...savePlan.map((candidate) => candidate.cubeId)];
      const forkedId = suggestPersonalCubeIdentity(forkedName, reservedIds).cubeId;
      const updatedMarkers = updateMarkersForIds(graph, markerIds, {
        cubeId: forkedId,
        defaultAlias: forkedName,
        cubeVersion: '',
        cubeRevisionRef: CURRENT_REVISION_REF,
      });
      if (!updatedMarkers) {
        throw new Error(`Unable to fork '${forkedName}' (markers missing).`);
      }
      for (const source of sourceEntries) {
        this.updateSourceGroupIdentity(source, {
          cubeId: forkedId,
          defaultAlias: forkedName,
          cubeVersion: '',
          cubeRevisionRef: CURRENT_REVISION_REF,
        });
      }
      entry.previousCubeId = entry.cubeId;
      entry.cubeId = forkedId;
      entry.forked = true;
      entry.reconciliationMarkerIds = markerIds;
      entry.lineage = this.buildHistoricalLineagePayload(browserEntry, entry);
      entry.metadata = this.buildSaveEntryMetadata({
        cubeId: forkedId,
        browserEntry,
      });
      entry.staleRevision = false;
      entry.sourceEntries = [];
      entry.sourceRevisionRef = '';
      entry.sourceVersion = '';
      entry.sourceDefinitionKey = '';
      entry.staleSaveMode = '';
      entry.latestVersion = '';
      entry.defaultAlias = forkedName;
    }
  }

  updateSourceGroupIdentity(source, { cubeId, defaultAlias, cubeVersion, cubeRevisionRef }) {
    const group = source?.group;
    const metadata = getGroupSugarcubes(group);
    if (!metadata) {
      return;
    }
    const nextVersion = normalizeCubeVersion(cubeVersion);
    setGroupSugarcubes(group, {
      ...metadata,
      cube_id: cubeId,
      default_alias: defaultAlias || metadata.default_alias || cubeId,
      cube_version: nextVersion,
      cube_revision_ref: normalizeRevisionRef(cubeRevisionRef),
      cube_definition_key: buildCubeDefinitionKey(cubeId, nextVersion),
    });
  }

  /** Resolve the exact graph instances that supplied each persisted save. */
  buildSaveReconciliationTargets(savePlan) {
    const targets = {};
    for (const entry of Array.isArray(savePlan) ? savePlan : []) {
      if (!entry?.cubeId) {
        continue;
      }
      if (entry.forked) {
        targets[entry.cubeId] = (entry.reconciliationMarkerIds || []).map(String);
      } else {
        const sources =
          entry.staleSaveMode === STALE_SAVE_MODE_LATEST
            ? (entry.sourceEntries || []).filter((source) => source?.staleRevision)
            : entry.selectedSourceEntry && !entry.selectedSourceEntry.staleRevision
              ? [entry.selectedSourceEntry]
              : [];
        targets[entry.cubeId] = Array.from(
          new Set(sources.flatMap((source) => source?.markerIds || []).map(String)),
        );
      }
    }
    return targets;
  }

  resolveCubeIds(cubeIds) {
    if (Array.isArray(cubeIds) && cubeIds.length) {
      return cubeIds.filter(Boolean);
    }
    const implementationDirty =
      typeof this.dirtyManager?.getImplementationDirtyCubeIds === 'function'
        ? Array.from(this.dirtyManager.getImplementationDirtyCubeIds() || [])
        : Array.from(this.dirtyManager?.getDirtyCubeIds?.() || []);
    if (implementationDirty.length) {
      return implementationDirty;
    }
    const dirty = Array.from(this.dirtyManager?.getDirtyCubeIds?.() || []);
    if (dirty.length) {
      return dirty;
    }
    return [];
  }

  async ensureCubeIds(graph) {
    if (!graph) {
      return { assigned: [], replacements: new Map() };
    }
    const builder =
      this.instanceManager?.instanceBuilder ||
      new InstanceBuilder({ logger: this.adapter?.getConsole?.() });
    const instances = builder.build(graph);
    const assigned = [];
    const replacements = new Map();
    const assignedIds = new Set();
    const reservedCubeIds = new Set(
      (this.cubeBrowser?.getCubes?.() || [])
        .map((cube) => (typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : ''))
        .filter(Boolean),
    );
    for (const instance of instances) {
      if (instance.cubeId && isCanonicalCubeId(instance.cubeId)) {
        reservedCubeIds.add(instance.cubeId);
      }
    }
    for (const instance of instances) {
      if (instance.cubeId && isCanonicalCubeId(instance.cubeId)) {
        continue;
      }
      const defaultAlias = instance.defaultAlias || 'SugarCube';
      const cubeId = suggestPersonalCubeIdentity(defaultAlias, Array.from(reservedCubeIds)).cubeId;
      reservedCubeIds.add(cubeId);
      if (instance.cubeId) {
        replacements.set(instance.cubeId, cubeId);
      }
      const updated = updateMarkersForIds(graph, instance.markerIds, { cubeId });
      if (updated) {
        assigned.push({ cubeId, instanceId: instance.instanceId });
        assignedIds.add(cubeId);
      }
    }
    const nodes = Array.isArray(graph._nodes) ? graph._nodes : graph.nodes || [];
    const fallbackByName = new Map();
    for (const node of nodes) {
      if (!isCubeMarkerType(node)) {
        continue;
      }
      const defaultAlias = readWidgetValue(node, 'default_alias');
      if (!defaultAlias) {
        continue;
      }
      const cubeId = readWidgetValue(node, 'cube_id');
      const entry = fallbackByName.get(defaultAlias) || {
        defaultAlias,
        cubeId: cubeId || '',
        markerIds: [],
      };
      if (cubeId && !entry.cubeId) {
        entry.cubeId = cubeId;
      }
      if (!cubeId) {
        entry.markerIds.push(node.id);
      }
      fallbackByName.set(defaultAlias, entry);
    }
    for (const entry of fallbackByName.values()) {
      if (!entry.markerIds.length) {
        continue;
      }
      const shouldReplace = entry.cubeId && !isCanonicalCubeId(entry.cubeId);
      const cubeId = shouldReplace
        ? suggestPersonalCubeIdentity(entry.defaultAlias, Array.from(reservedCubeIds)).cubeId
        : entry.cubeId;
      if (shouldReplace) {
        replacements.set(entry.cubeId, cubeId);
        reservedCubeIds.add(cubeId);
      }
      if (!cubeId) {
        const personalId = suggestPersonalCubeIdentity(
          entry.defaultAlias,
          Array.from(reservedCubeIds),
        ).cubeId;
        reservedCubeIds.add(personalId);
        const updated = updateMarkersForIds(graph, entry.markerIds, { cubeId: personalId });
        if (updated && !assignedIds.has(personalId)) {
          assigned.push({ cubeId: personalId, instanceId: null });
          assignedIds.add(personalId);
        }
        continue;
      }
      const updated = updateMarkersForIds(graph, entry.markerIds, { cubeId });
      if (updated && !assignedIds.has(cubeId)) {
        assigned.push({ cubeId, instanceId: null });
        assignedIds.add(cubeId);
      }
    }
    if (assigned.length) {
      this.instanceManager?.scheduleRefresh?.({ graph, reason: 'cube-id-assigned' });
      this.dirtyManager?.scheduleRefresh?.({ graph, reason: 'cube-id-assigned' });
    }
    return { assigned, replacements };
  }

  pushToastMessage(severity, summary, detail) {
    this.toast?.push?.(severity, summary, detail);
  }

  isWritableEntry(entry) {
    return Boolean(entry?.is_writable);
  }

  buildForkedName(baseName, usedNames) {
    const base = typeof baseName === 'string' && baseName.trim() ? baseName.trim() : 'SugarCube';
    const used = usedNames instanceof Set ? usedNames : new Set();
    const fallback = `${base} (fork)`;
    if (!used.has(fallback)) {
      return fallback;
    }
    let index = 2;
    while (index < 1000) {
      const next = `${base} (fork ${index})`;
      if (!used.has(next)) {
        return next;
      }
      index += 1;
    }
    return `${base} (fork ${Date.now()})`;
  }

  buildLineagePayload(entry) {
    if (!entry || typeof entry !== 'object') {
      return null;
    }
    return {
      id: entry.cube_id || '',
      name: entry.name || '',
      version: entry.version || '',
      author: entry.author || '',
      author_url: entry.author_url || '',
      forked_at: new Date().toISOString(),
    };
  }

  buildHistoricalLineagePayload(browserEntry, saveEntry) {
    const entry = saveEntry && typeof saveEntry === 'object' ? saveEntry : {};
    const source = browserEntry && typeof browserEntry === 'object' ? browserEntry : {};
    return {
      id: entry.previousCubeId || source.cube_id || '',
      name: entry.defaultAlias || source.name || '',
      version: entry.sourceVersion || source.version || '',
      revision_ref: entry.sourceRevisionRef || '',
      author: source.author || '',
      author_url: source.author_url || '',
      forked_at: new Date().toISOString(),
    };
  }

  buildCubeEntryIndex(entries) {
    const map = new Map();
    const list = Array.isArray(entries) ? entries : [];
    for (const entry of list) {
      const cubeId = typeof entry?.cube_id === 'string' ? entry.cube_id.trim() : '';
      if (!cubeId) {
        continue;
      }
      map.set(cubeId, entry);
    }
    return map;
  }
}
