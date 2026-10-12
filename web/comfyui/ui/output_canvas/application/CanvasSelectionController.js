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
import { CanvasResultPins } from './CanvasResultPins.js';
/** Resolve an exact slot without substituting a neighboring batch image. */
export function canvasSelectedItem(catalog, selection) {
    if (!selection)
        return null;
    return (catalog.entries
        .find((entry) => entry.source.id === selection.sourceId)
        ?.items.find((item) => item.batchIndex === selection.batchIndex) ?? null);
}
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
/** Keep per-view selection policy separate from immutable registry storage and result ingestion. */
export class CanvasSelectionController {
    views;
    #cursors = new Map();
    #pins = new CanvasResultPins();
    /** Use the existing view authority for every published state transition. */
    constructor(views) {
        this.views = views;
    }
    /** Reconcile a named view without making background deliveries select another workflow. */
    reconcile(viewId, catalog) {
        let state = this.views.read(viewId);
        if (state.sessionId !== catalog.sessionId)
            throw new TypeError('Canvas catalog belongs to another workflow.');
        let cursor = this.#cursors.get(viewId);
        if (!cursor || cursor.generation !== catalog.generation) {
            const replacement = Boolean(cursor);
            cursor = { generation: catalog.generation, frontier: null };
            this.#cursors.set(viewId, cursor);
            if (replacement) {
                this.#pins.clear(viewId);
                state = this.views.update(viewId, (current) => ({
                    ...current,
                    selection: null,
                    sourcePreview: true,
                    comparison: null,
                    follow: 'auto',
                    mode: 'image',
                }));
            }
        }
        if (state.follow !== 'auto')
            return state;
        const candidate = autoCandidate(catalog);
        const currentSourceExists = catalog.entries.some((entry) => entry.source.id === state.selection?.sourceId);
        if (!candidate ||
            (currentSourceExists && cursor.frontier && !later(candidate.frontier, cursor.frontier)))
            return state;
        // An older run can remain visible, but cannot displace the frontier after a new run begins.
        if (state.selection && candidate.frontier[0] < catalog.result.runOrder)
            return state;
        cursor.frontier = candidate.frontier;
        if (state.selection?.sourceId === candidate.selection.sourceId &&
            state.selection.batchIndex === candidate.selection.batchIndex)
            return state;
        return this.views.update(viewId, (current) => ({
            ...current,
            selection: candidate.selection,
            sourcePreview: true,
        }));
    }
    /** Apply navigation intents; rendering and resource commands belong to their own owners. */
    apply(viewId, catalog, command) {
        this.reconcile(viewId, catalog);
        const state = this.views.read(viewId);
        switch (command.type) {
            case 'select':
                this.#select(viewId, catalog, command.selection, false, false);
                return true;
            case 'select-source':
                this.#select(viewId, catalog, command.selection, false, true);
                return true;
            case 'batch':
                if (state.selection)
                    this.#select(viewId, catalog, { ...state.selection, batchIndex: command.index }, false, false);
                return true;
            case 'follow':
                if (command.mode === 'auto') {
                    this.#pins.clear(viewId);
                    this.#cursors.set(viewId, { generation: catalog.generation, frontier: null });
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
                this.views.update(viewId, (current) => ({
                    ...current,
                    follow: 'manual',
                    mode: command.enabled ? 'overview' : 'image',
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
                    this.#select(viewId, catalog, command.selection, true, false);
                return true;
            case 'swap': {
                const comparison = state.comparison;
                if (comparison?.secondary && state.selection) {
                    const pair = this.#pins.read(viewId);
                    if (pair)
                        this.#pins.set(viewId, { primary: pair.secondary, secondary: pair.primary });
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
    /** Suspend following as soon as a user starts an inspection gesture. */
    manual(viewId) {
        this.views.update(viewId, (state) => state.follow === 'manual' ? state : { ...state, follow: 'manual' });
    }
    /** Read pinned revisions only while their current workflow source still exists. */
    items(viewId, catalog) {
        const state = this.views.read(viewId);
        const pinned = state.comparison ? this.#pins.read(viewId) : null;
        const valid = (selection, item) => catalog.entries.some((entry) => entry.source.id === selection?.sourceId) ? item : null;
        return {
            primary: pinned
                ? valid(state.selection, pinned.primary)
                : canvasSelectedItem(catalog, state.selection),
            secondary: pinned ? valid(state.comparison?.secondary ?? null, pinned.secondary) : null,
        };
    }
    /** Admit source-level preview interest without assigning unknown preview bytes to an explicit batch. */
    allowsSourcePreview(viewId, catalog, source) {
        const state = this.views.read(viewId);
        return (state.sessionId === catalog.sessionId &&
            this.#cursors.get(viewId)?.generation === catalog.generation &&
            state.mode === 'image' &&
            state.comparison === null &&
            state.sourcePreview &&
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
        this.#pins.clear(viewId);
    }
    /** End this integration without retaining lightweight comparison history. */
    dispose() {
        this.#cursors.clear();
        this.#pins.dispose();
    }
    /** Preserve exact requested slots, including explicit unavailable states. */
    #select(viewId, catalog, selection, secondary, sourcePreview) {
        if (!Number.isSafeInteger(selection.batchIndex) || selection.batchIndex < 0)
            throw new TypeError('Canvas batch index is invalid.');
        const state = this.views.read(viewId);
        if (state.comparison) {
            const pair = this.#pins.read(viewId) ?? { primary: null, secondary: null };
            this.#pins.set(viewId, secondary
                ? { ...pair, secondary: canvasSelectedItem(catalog, selection) }
                : { ...pair, primary: canvasSelectedItem(catalog, selection) });
        }
        this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            mode: 'image',
            selection: secondary ? current.selection : selection,
            sourcePreview: secondary ? current.sourcePreview : sourcePreview,
            comparison: secondary && current.comparison
                ? { ...current.comparison, secondary: selection }
                : current.comparison,
        }));
    }
    /** Pin only final still images, with an empty chooser when no distinct B is available. */
    #compare(viewId, catalog, enabled) {
        const state = this.views.read(viewId);
        if (!enabled) {
            this.#pins.clear(viewId);
            this.views.update(viewId, (current) => ({ ...current, comparison: null, follow: 'manual' }));
            return;
        }
        if (state.comparison)
            return;
        const primary = canvasSelectedItem(catalog, state.selection);
        const secondary = catalog.entries
            .flatMap((entry) => entry.items)
            .find((item) => item.mediaKind === 'image' && item.id !== primary?.id) ?? null;
        this.#pins.set(viewId, { primary, secondary });
        this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            mode: 'image',
            comparison: {
                secondary: secondary
                    ? { sourceId: secondary.provenance.sourceId, batchIndex: secondary.batchIndex }
                    : null,
                orientation: 'vertical',
                split: 0.5,
            },
        }));
    }
}
