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
/** Own navigation, execution following, and bounded comparison revision intent. */
import { resolveCanvasReveal } from '../domain/CanvasReveal.js';
import { canvasOutputCatalog } from './CanvasOutputCatalog.js';
import { canvasComparisonPair } from './CanvasComparisonSelection.js';
import { canvasSelectedItem, canvasAvailableSelection, canvasItemSelection, canvasBatchGridCatalog, } from './CanvasContentSelection.js';
/** Compare attributable execution progress independently of delivery order. */
function later(left, right) {
    for (let index = 0; index < left.length; index++) {
        const a = left[index] ?? 0;
        const b = right[index] ?? 0;
        if (a !== b)
            return a > b;
    }
    return false;
}
/** Find the newest authored execution frontier, including a source-level live preview. */
function autoCandidate(catalog) {
    let candidate = null;
    for (const entry of catalog.entries) {
        for (const item of entry.items) {
            const frontier = [
                item.provenance.runOrder,
                entry.source.order,
                1,
                item.provenance.revision,
            ];
            if (!candidate || later(frontier, candidate.frontier))
                candidate = {
                    selection: { sourceId: entry.source.id, batchIndex: item.batchIndex },
                    frontier,
                };
        }
        if (entry.preview) {
            const frontier = [
                entry.preview.runOrder,
                entry.source.order,
                0,
                entry.preview.sequence,
            ];
            if (!candidate || later(frontier, candidate.frontier))
                candidate = { selection: { sourceId: entry.source.id, batchIndex: 0 }, frontier };
        }
    }
    return candidate;
}
/** Return the current scene run when it has several scenes to choose between. */
function sceneRun(catalog) {
    const run = catalog.result.scenes;
    return run && run.scenes.length > 1 ? run : null;
}
/** Open one scene at its first source's first image, or its batch grid when that has several. */
function sceneOpening(catalog, scene, running) {
    const outputs = new Map(scene.outputs.map((output) => [output.source.id, output]));
    for (const source of [...catalog.sources].sort((a, b) => a.order - b.order)) {
        const output = outputs.get(source.id);
        const item = output?.items[0];
        if (output && item)
            return {
                selection: { sourceId: source.id, batchIndex: item.batchIndex },
                mode: output.batchCount > 1 ? 'overview' : 'image',
            };
    }
    const preview = running ? catalog.result.preview : null;
    return preview
        ? { selection: { sourceId: preview.sourceId, batchIndex: 0 }, mode: 'image' }
        : null;
}
/** Keep per-view selection policy separate from immutable registry storage and result ingestion. */
export class CanvasSelectionController {
    views;
    submissions;
    #cursors = new Map();
    /** Use the existing view authority for every published state transition. */
    constructor(views, submissions) {
        this.views = views;
        this.submissions = submissions;
    }
    /** Follow current Run content independently of finals retained for manual inspection. */
    reconcile(viewId, catalog) {
        let state = this.views.read(viewId);
        if (state.sessionId !== catalog.sessionId)
            throw new TypeError('Canvas catalog belongs to another workflow.');
        let cursor = this.#cursors.get(viewId);
        if (!cursor || cursor.generation !== catalog.generation) {
            const replacement = Boolean(cursor);
            cursor = { generation: catalog.generation, frontier: null, presentedRun: 0 };
            this.#cursors.set(viewId, cursor);
            if (replacement) {
                state = this.views.update(viewId, (current) => ({
                    ...current,
                    selection: null,
                    sourcePreview: true,
                    comparison: null,
                    follow: 'auto',
                    mode: 'image',
                    scene: null,
                }));
            }
        }
        const scenes = sceneRun(catalog);
        if (!scenes && state.scene !== null)
            state = this.views.update(viewId, (current) => ({ ...current, scene: null }));
        const automatic = canvasOutputCatalog(catalog.result, catalog.sources);
        const candidate = autoCandidate(automatic);
        if (candidate && candidate.frontier[0] > cursor.presentedRun) {
            cursor.presentedRun = candidate.frontier[0];
            cursor.frontier = null;
            state = this.views.update(viewId, (current) => ({
                ...current,
                follow: 'auto',
                sourcePreview: true,
                comparison: null,
                mode: 'image',
            }));
        }
        if (state.selection && state.sourcePreview) {
            const activeCatalog = catalog;
            const entry = activeCatalog.entries.find((entry) => entry.source.id === state.selection?.sourceId);
            if (!entry?.preview) {
                const available = canvasAvailableSelection(activeCatalog, state.selection);
                if (available.batchIndex !== state.selection.batchIndex)
                    state = this.views.update(viewId, (current) => ({ ...current, selection: available }));
            }
        }
        if (state.follow !== 'auto')
            return state;
        const currentSourceExists = catalog.entries.some((entry) => entry.source.id === state.selection?.sourceId);
        if (!candidate ||
            (currentSourceExists && cursor.frontier && !later(candidate.frontier, cursor.frontier)))
            return state;
        // An older run can remain visible, but cannot displace the frontier after a new run begins.
        if (state.selection && candidate.frontier[0] < catalog.result.runOrder)
            return state;
        cursor.frontier = candidate.frontier;
        // A scene run opens on every scene together, as Substitute's scene overview did.
        if (scenes) {
            if (state.scene === null && state.mode === 'overview')
                return state;
            return this.views.update(viewId, (current) => ({
                ...current,
                selection: candidate.selection,
                scene: null,
                mode: 'overview',
                sourcePreview: true,
            }));
        }
        const batch = canvasBatchGridCatalog(automatic, candidate.selection, true);
        const mode = (batch.entries[0]?.summary.batchCount ?? 0) > 1 ? 'overview' : 'image';
        if (state.selection?.sourceId === candidate.selection.sourceId &&
            state.selection.batchIndex === candidate.selection.batchIndex &&
            state.mode === mode)
            return state;
        return this.views.update(viewId, (current) => ({
            ...current,
            selection: candidate.selection,
            mode,
            sourcePreview: true,
        }));
    }
    /** Apply navigation intents; rendering and resource commands belong to their own owners. */
    apply(viewId, catalog, command) {
        this.reconcile(viewId, catalog);
        const state = this.views.read(viewId);
        switch (command.type) {
            case 'reveal': {
                const item = resolveCanvasReveal(catalog, command.target);
                if (!item)
                    return false;
                const cursor = this.#cursors.get(viewId);
                // Explicit inspection during an accepted job also owns its not-yet-presented first frame.
                if (cursor)
                    cursor.presentedRun = Math.max(cursor.presentedRun, catalog.result.runOrder, this.submissions.latestOrder(item.provenance.sessionId));
                this.views.update(viewId, (current) => ({
                    ...current,
                    follow: 'manual',
                    mode: 'image',
                    sourcePreview: false,
                    selection: { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex },
                    comparison: null,
                    scene: sceneRun(catalog)?.scenes.find((scene) => scene.outputs.some((output) => output.items.some((entry) => entry.id === item.id)))?.key ?? null,
                }));
                return true;
            }
            case 'select':
                this.#select(viewId, catalog, command.selection, 'image');
                return true;
            case 'select-source':
                this.#select(viewId, catalog, command.selection.sourceId === state.selection?.sourceId
                    ? command.selection
                    : canvasAvailableSelection(catalog, command.selection), 'source');
                return true;
            case 'select-preview':
                this.#select(viewId, catalog, { sourceId: command.sourceId, batchIndex: 0 }, 'preview');
                return true;
            case 'select-scene': {
                const run = sceneRun(catalog);
                if (!run)
                    return true;
                if (command.scene === null) {
                    this.views.update(viewId, (current) => ({
                        ...current,
                        follow: 'manual',
                        scene: null,
                        mode: 'overview',
                        sourcePreview: true,
                        comparison: null,
                    }));
                    return true;
                }
                const scene = run.scenes.find((entry) => entry.key === command.scene);
                const opening = scene ? sceneOpening(catalog, scene, run.running === scene.key) : null;
                if (!scene || !opening)
                    return true;
                this.views.update(viewId, (current) => ({
                    ...current,
                    follow: 'manual',
                    scene: scene.key,
                    mode: opening.mode,
                    selection: opening.selection,
                    sourcePreview: true,
                    comparison: null,
                }));
                return true;
            }
            case 'batch': {
                if (!state.selection)
                    return true;
                const selection = { ...state.selection, batchIndex: command.index };
                const preview = command.index === 0 &&
                    catalog.entries.some((entry) => entry.source.id === selection.sourceId && entry.preview);
                if (preview || canvasSelectedItem(catalog, selection))
                    this.#select(viewId, catalog, selection, preview ? 'preview' : 'image');
                return true;
            }
            case 'follow':
                if (command.mode === 'auto') {
                    this.#cursors.set(viewId, {
                        generation: catalog.generation,
                        frontier: null,
                        presentedRun: this.#cursors.get(viewId)?.presentedRun ?? 0,
                    });
                    this.views.update(viewId, (current) => ({
                        ...current,
                        follow: 'auto',
                        sourcePreview: true,
                        comparison: null,
                        mode: 'image',
                    }));
                    this.reconcile(viewId, catalog);
                }
                else
                    this.manual(viewId);
                return true;
            case 'overview':
                if (command.enabled &&
                    (!state.selection ||
                        !catalog.entries.some((entry) => entry.source.id === state.selection?.sourceId &&
                            (entry.items.length || entry.preview))))
                    return true;
                this.views.update(viewId, (current) => ({
                    ...current,
                    follow: 'manual',
                    mode: command.enabled ? 'overview' : 'image',
                    sourcePreview: command.enabled || current.sourcePreview,
                    comparison: null,
                }));
                return true;
            case 'link':
                this.views.update(viewId, (current) => ({ ...current, linked: command.enabled }));
                return true;
            case 'comparison':
                this.#compare(viewId, catalog, command.enabled);
                return true;
            case 'select-comparison':
                if (state.comparison)
                    this.#select(viewId, catalog, command.selection, 'comparison');
                return true;
            case 'swap': {
                const comparison = state.comparison;
                if (comparison?.secondary && state.selection) {
                    this.views.update(viewId, (current) => ({
                        ...current,
                        follow: 'manual',
                        selection: comparison.secondary,
                        sourcePreview: false,
                        comparison: { ...comparison, secondary: state.selection },
                    }));
                }
                return true;
            }
            case 'wipe': {
                const comparison = state.comparison;
                if (comparison)
                    this.views.update(viewId, (current) => ({
                        ...current,
                        follow: 'manual',
                        comparison: {
                            ...comparison,
                            split: command.split,
                            orientation: command.orientation,
                        },
                    }));
                return true;
            }
            default:
                return false;
        }
    }
    /** Hold deliberate content navigation for the current output session. */
    manual(viewId) {
        this.views.update(viewId, (state) => state.follow === 'manual' ? state : { ...state, follow: 'manual' });
    }
    /** Restore navigation and its observed frontier together so imported finals do not act like a new Run. */
    restore(viewId, catalog, restored) {
        const candidate = autoCandidate(canvasOutputCatalog(catalog.result, catalog.sources));
        this.#cursors.set(viewId, {
            generation: catalog.generation,
            frontier: candidate?.frontier ?? null,
            presentedRun: catalog.result.presentedRunOrder,
        });
        this.views.update(viewId, (state) => ({ ...state, ...restored }));
    }
    /** Keep an explicitly inspected source available independently of automatic Run handoff. */
    inspection(viewId) {
        const state = this.views.read(viewId);
        return state.follow === 'manual' && !state.sourcePreview ? state.selection : null;
    }
    /** Resolve both semantic selections to current finals without retaining superseded revisions. */
    items(viewId, catalog) {
        const state = this.views.read(viewId);
        return {
            primary: canvasSelectedItem(catalog, state.selection),
            secondary: canvasSelectedItem(catalog, state.comparison?.secondary ?? null),
        };
    }
    /** Admit source-level preview interest without assigning unknown preview bytes to an explicit batch. */
    allowsSourcePreview(viewId, catalog, source) {
        const state = this.views.read(viewId);
        return (state.sessionId === catalog.sessionId &&
            this.#cursors.get(viewId)?.generation === catalog.generation &&
            (state.mode === 'image' ||
                (state.mode === 'overview' &&
                    (catalog.result.runStatus === 'running' || catalog.result.runStatus === 'queued'))) &&
            state.comparison === null &&
            state.sourcePreview &&
            (state.mode === 'overview' || state.selection?.batchIndex === 0) &&
            state.selection?.sourceId === source &&
            catalog.entries.some((entry) => entry.source.id === source));
    }
    /** Read qualified transient metadata for the selected visible source without retaining its bytes. */
    preview(viewId, catalog) {
        const preview = catalog.result.preview;
        return preview && this.allowsSourcePreview(viewId, catalog, preview.sourceId) ? preview : null;
    }
    /** Release all selection-owned references when a view closes. */
    remove(viewId) {
        this.#cursors.delete(viewId);
    }
    /** Release navigation state at integration teardown. */
    dispose() {
        this.#cursors.clear();
    }
    /** Preserve exact requested slots, including explicit unavailable states. */
    #select(viewId, catalog, selection, intent) {
        if (!Number.isSafeInteger(selection.batchIndex) || selection.batchIndex < 0)
            throw new TypeError('Canvas batch index is invalid.');
        const state = this.views.read(viewId);
        const secondary = intent === 'comparison';
        const media = canvasSelectedItem(catalog, selection);
        if (secondary && media && media.mediaKind !== 'image')
            return;
        const sourcePreview = intent === 'source' || intent === 'preview';
        const overview = intent === 'source' && state.mode === 'overview';
        if (intent === 'preview' &&
            !catalog.entries.some((entry) => entry.source.id === selection.sourceId && entry.preview))
            return;
        if (sourcePreview || state.comparison) {
            const entry = catalog.entries.find((entry) => entry.source.id === selection.sourceId);
            if (!entry)
                return;
            if (overview) {
                if (!entry.items.length && !entry.preview)
                    return;
            }
            else if (!canvasSelectedItem(catalog, selection)) {
                if (!sourcePreview || !entry.preview || selection.batchIndex !== 0)
                    return;
            }
        }
        this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            mode: overview ? 'overview' : 'image',
            selection: secondary ? current.selection : selection,
            sourcePreview: secondary ? current.sourcePreview : sourcePreview,
            comparison: secondary && current.comparison
                ? { ...current.comparison, secondary: selection }
                : media && media.mediaKind !== 'image'
                    ? null
                    : current.comparison,
        }));
    }
    /** Start reveal comparison with the first and last final stages in the selected batch. */
    #compare(viewId, catalog, enabled) {
        const state = this.views.read(viewId);
        if (!enabled) {
            this.views.update(viewId, (current) => ({ ...current, comparison: null, follow: 'manual' }));
            return;
        }
        if (state.comparison)
            return;
        const pair = canvasComparisonPair(catalog, state.selection?.batchIndex ?? 0);
        if (!pair?.primary || !pair.secondary)
            return;
        const { primary, secondary } = pair;
        this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            mode: 'image',
            selection: canvasItemSelection(primary),
            sourcePreview: false,
            comparison: {
                secondary: canvasItemSelection(secondary),
                orientation: 'vertical',
                split: 0.5,
            },
        }));
    }
}
