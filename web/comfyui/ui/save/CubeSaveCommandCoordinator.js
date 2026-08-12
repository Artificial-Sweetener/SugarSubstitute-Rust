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
/** Coordinate backend save execution and user-facing outcomes. */
import { isCanonicalCubeId } from '../core/CubeId.js';
import { normalizeCubeVersion } from '../core/CubeDefinitionKey.js';
import { updateMarkersForCubeId } from '../graph/CubeMarkers.js';
import { enrichWorkflowPayload } from '../graph/WorkflowPayloadBuilder.js';
import { suggestPersonalCubeIdentity } from '../create/PersonalCubeIdentity.js';
import { SugarCubeExportError, buildErrorDetail, formatSaveSummaryEntry, formatViolations, } from './SaveFeedback.js';
import { isRecord } from '../types/common.js';
import { STALE_SAVE_MODE_LATEST, } from './CubeSaveContracts.js';
/** Own the transactional application workflow for one save command. */
export class CubeSaveCommandCoordinator {
    options;
    dependencies;
    constructor(options) {
        this.options = options;
        this.dependencies = options.dependencies;
    }
    /** Execute a save while retaining errors and feedback at the command boundary. */
    async execute({ cubeIds = null, button = null } = {}) {
        const setBusy = typeof button?.enabled === 'boolean'
            ? (busy) => {
                button.enabled = !busy;
                button.element?.classList?.toggle('sugarcubes-save--busy', Boolean(busy));
            }
            : (_busy) => { };
        setBusy(true);
        try {
            const appRef = this.dependencies.adapter.getApp?.();
            const graph = appRef?.graph;
            if (!graph)
                throw new Error('Unable to access the current graph');
            const { assigned, replacements } = await this.options.identities.ensureCubeIds(graph);
            const assignedIds = assigned.map((entry) => entry.cubeId);
            const targetCubeIds = this.resolveCubeIds(cubeIds);
            const replacementIds = Array.from(replacements.values());
            const cubeEntries = Array.isArray(this.dependencies.cubeBrowser?.getCubes?.())
                ? this.dependencies.cubeBrowser.getCubes()
                : [];
            const cubeIndex = this.options.sources.indexBrowserEntries(cubeEntries);
            const sourceMetadataIndex = this.options.sources.buildIndex(graph);
            const usedNames = new Set(cubeEntries
                .map((entry) => (typeof entry.name === 'string' ? entry.name : ''))
                .filter(Boolean));
            const savePlan = this.buildSavePlan(graph, targetCubeIds, assignedIds, replacementIds, replacements, cubeIndex, sourceMetadataIndex, usedNames);
            if (!savePlan.length) {
                this.pushToast('info', 'No changes', 'No dirty SugarCubes to save.');
                return { status: 'no_changes', savedCubeIds: [] };
            }
            const historicalChoice = await this.options.forks.resolveHistoricalChoice(savePlan);
            if (historicalChoice === null)
                return { status: 'cancelled', savedCubeIds: [] };
            if (historicalChoice === 'fork') {
                await this.options.forks.forkStaleEntries(graph, savePlan, cubeIndex, usedNames);
            }
            else if (historicalChoice === STALE_SAVE_MODE_LATEST) {
                for (const entry of savePlan) {
                    if (entry.staleRevision)
                        entry.staleSaveMode = STALE_SAVE_MODE_LATEST;
                }
            }
            if (savePlan.some((entry) => entry.forked)) {
                this.dependencies.instanceManager?.scheduleRefresh?.({ graph, reason: 'fork' });
                this.dependencies.dirtyManager?.scheduleRefresh?.({ graph, reason: 'fork' });
            }
            const promptResult = appRef.graphToPrompt?.();
            const resolved = isPromiseLike(promptResult) ? await promptResult : promptResult;
            const resolvedRecord = isRecord(resolved) ? resolved : {};
            const graphPayload = resolvedRecord.output ?? resolvedRecord.prompt ?? resolved;
            if (!isRecord(graphPayload))
                throw new Error('Unable to serialize the current graph');
            const workflowPayload = resolvedRecord.workflow ?? null;
            if (!isRecord(workflowPayload) || Array.isArray(workflowPayload)) {
                throw new Error('Workflow payload unavailable');
            }
            const enrichedWorkflowPayload = enrichWorkflowPayload(workflowPayload, graph);
            const requestBody = {
                graph: graphPayload,
                cubes: savePlan.map((entry) => this.buildRequestEntry(entry)),
                workflow: enrichedWorkflowPayload,
                workflow_version: enrichedWorkflowPayload.version ?? null,
            };
            const reviewedRequest = this.dependencies.defaultReview
                ? await this.dependencies.defaultReview.review(requestBody)
                : requestBody;
            if (reviewedRequest === null)
                return { status: 'cancelled', savedCubeIds: [] };
            const { response, data } = await this.dependencies.api.saveImplementation(JSON.stringify(reviewedRequest), { headers: { 'Content-Type': 'application/json' } });
            const errorPayload = isRecord(data.error) ? data.error : null;
            if (!response.ok || errorPayload) {
                const message = (typeof errorPayload?.message === 'string' ? errorPayload.message : '') ||
                    response.statusText ||
                    'Export failed';
                throw new SugarCubeExportError(message, buildErrorDetail(errorPayload), errorPayload?.violations);
            }
            const saved = Array.isArray(data.saved)
                ? data.saved.filter((entry) => isRecord(entry))
                : [];
            const summary = saved.length
                ? saved.map((entry) => formatSaveSummaryEntry(toSaveSummaryEntry(entry))).join('\n')
                : 'No cubes were saved';
            this.pushToast('success', 'SugarCubes exported', summary);
            const warnings = Array.isArray(data.warnings)
                ? data.warnings.filter(Boolean).map(String)
                : [];
            if (warnings.length)
                this.pushToast('warn', 'SugarCubes warnings', warnings.join('\n'));
            const versionSuggestions = Array.isArray(data.version_suggestions)
                ? data.version_suggestions
                : [];
            if (versionSuggestions.length) {
                await this.dependencies.versionDialog?.open?.(versionSuggestions);
            }
            const savedIds = saved
                .map((entry) => (typeof entry.cube_id === 'string' ? entry.cube_id : ''))
                .filter(Boolean);
            if (!savedIds.length) {
                this.pushToast('info', 'No changes', 'The save operation did not persist any SugarCubes.');
                return { status: 'no_changes', savedCubeIds: [] };
            }
            if (!this.dependencies.saveReconciler?.reconcile) {
                throw new Error('Cube save reconciler is unavailable');
            }
            await this.dependencies.saveReconciler.reconcile({
                graph,
                saved,
                fallbackCubeIds: savedIds,
                markerIdsByCubeId: this.options.reconciliationTargets.buildMarkerTargets(savePlan),
                cubeNodeInstanceIdsByCubeId: this.options.reconciliationTargets.buildCubeNodeTargets(savePlan),
                reason: 'save',
            });
            await this.dependencies.catalogInvalidator?.invalidate();
            return { status: 'saved', savedCubeIds: savedIds };
        }
        catch (error) {
            const exportError = error instanceof SugarCubeExportError ? error : SugarCubeExportError.from(error);
            const detail = exportError.detail || formatViolations(exportError.violations);
            this.pushToast('error', exportError.message, detail);
            this.dependencies.adapter.getConsole?.()?.error?.(exportError.message);
            return { status: 'failed', savedCubeIds: [], message: exportError.message };
        }
        finally {
            setBusy(false);
        }
    }
    buildSavePlan(graph, targetCubeIds, assignedIds, replacementIds, replacements, cubeIndex, sourceMetadataIndex, usedNames) {
        const savePlan = [];
        const previousIdByCubeId = new Map();
        for (const [previousCubeId, nextCubeId] of replacements.entries()) {
            if (previousCubeId.trim() && nextCubeId)
                previousIdByCubeId.set(nextCubeId, previousCubeId.trim());
        }
        const finalCubeIds = Array.from(new Set([
            ...targetCubeIds.filter((cubeId) => isCanonicalCubeId(cubeId) && !replacements.has(cubeId)),
            ...assignedIds,
            ...replacementIds,
        ]));
        for (const cubeId of finalCubeIds) {
            const entry = cubeIndex.get(cubeId);
            const sourceMetadata = this.options.sources.resolve(cubeId, sourceMetadataIndex);
            const metadata = this.options.sources.buildMetadata(cubeId, entry ?? null, sourceMetadata);
            if (!entry || entry.is_writable) {
                savePlan.push({
                    cubeId,
                    forked: false,
                    lineage: null,
                    metadata,
                    previousCubeId: previousIdByCubeId.get(cubeId) || '',
                    latestVersion: entry ? normalizeCubeVersion(entry.version) : '',
                    ...sourceMetadata,
                });
                continue;
            }
            const forkedName = this.options.forks.buildForkedName(entry.name || 'SugarCube', usedNames);
            usedNames.add(forkedName);
            const reservedIds = [...cubeIndex.keys(), ...savePlan.map((candidate) => candidate.cubeId)];
            const forkedId = suggestPersonalCubeIdentity(forkedName, this.options.identities.resolveTargetModel(entry.target_model, sourceMetadata.targetModel, cubeId), reservedIds).cubeId;
            const updatedMarkers = updateMarkersForCubeId(graph, cubeId, {
                cubeId: forkedId,
                defaultAlias: forkedName,
            });
            const cubeNodeInstanceIds = sourceMetadata.sourceEntries
                .map((source) => source.cubeNodeInstanceId)
                .filter((instanceId) => Boolean(instanceId));
            const updatedCubeNodes = this.dependencies.cubeNodeSave?.updateIdentities(cubeNodeInstanceIds, {
                cubeId: forkedId,
                defaultAlias: forkedName,
            }) ?? 0;
            if (!updatedMarkers && !updatedCubeNodes) {
                throw new Error(`Unable to fork cube '${entry.name || cubeId}' (instances missing).`);
            }
            savePlan.push({
                cubeId: forkedId,
                forked: true,
                lineage: this.options.forks.buildLineage(entry),
                metadata: this.options.sources.buildMetadata(forkedId, { ...entry, target_model: '', supported_models: [] }, sourceMetadata),
                previousCubeId: cubeId,
                staleRevision: false,
                sourceEntries: sourceMetadata.sourceEntries,
                sourceRevisionRef: '',
                sourceVersion: '',
                sourceDefinitionKey: '',
                staleSaveMode: '',
                selectedSourceEntry: sourceMetadata.selectedSourceEntry,
                defaultAlias: forkedName,
                targetModel: '',
                supportedModels: [],
                latestVersion: normalizeCubeVersion(entry.version),
                reconciliationCubeNodeInstanceIds: cubeNodeInstanceIds,
            });
        }
        return savePlan;
    }
    buildRequestEntry(entry) {
        return {
            cube_id: entry.cubeId,
            forked: entry.forked,
            lineage: entry.lineage,
            previous_cube_id: entry.previousCubeId,
            source_revision_ref: entry.sourceRevisionRef || '',
            source_version: entry.sourceVersion || '',
            source_definition_key: entry.sourceDefinitionKey || '',
            stale_save_mode: entry.staleSaveMode || '',
            ...(entry.selectedSourceEntry?.description != null
                ? { description: entry.selectedSourceEntry.description }
                : {}),
            ...(entry.selectedSourceEntry?.definitionId
                ? {
                    definition_id: entry.selectedSourceEntry.definitionId,
                    instance_node_ids: entry.sourceEntries
                        .filter((source) => source.definitionId === entry.selectedSourceEntry?.definitionId &&
                        source.cubeNodeId != null)
                        .map((source) => String(source.cubeNodeId)),
                }
                : {}),
            ...(entry.metadata ? { metadata: entry.metadata } : {}),
        };
    }
    resolveCubeIds(cubeIds) {
        if (Array.isArray(cubeIds) && cubeIds.length)
            return cubeIds.filter(Boolean);
        const dirtyManager = this.dependencies.dirtyManager;
        const implementationDirty = typeof dirtyManager?.getImplementationDirtyCubeIds === 'function'
            ? Array.from(dirtyManager.getImplementationDirtyCubeIds() || [])
            : Array.from(dirtyManager?.getDirtyCubeIds?.() || []);
        if (implementationDirty.length)
            return implementationDirty;
        return Array.from(dirtyManager?.getDirtyCubeIds?.() || []);
    }
    pushToast(severity, summary, detail) {
        this.dependencies.toast?.push?.(severity, summary, detail);
    }
}
function isPromiseLike(value) {
    return isRecord(value) && typeof value.then === 'function';
}
function toSaveSummaryEntry(entry) {
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
