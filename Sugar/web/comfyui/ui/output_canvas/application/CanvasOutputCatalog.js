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
/** Project current source membership and authoritative execution results into stable navigation. */
import { canvasSessionId } from '../domain/CanvasIdentity.js';
/** Retain displayed workflow media until the first visual hands over to the current Run. */
export function canvasOutputCatalog(result, inventory, inspection = null) {
    const presentedRun = latestRun(result.outputs, Math.max(result.presentedRunOrder, result.preview?.runOrder ?? 0));
    return catalog(result, inventory, sourceEntries(result, result.outputs, result.preview, presentedRun, inventory, inspection));
}
/**
 * Project one scene of the current scene run, or every scene when `scene` is null.
 *
 * A chosen scene shows its own latest outputs and, while it is the running scene, the live
 * preview. Every scene together shows one representative per scene that has something to show:
 * its first source's first image, or the running scene's live preview. Results without several
 * scenes project as they do without scenes.
 */
export function canvasSceneCatalog(result, inventory, inspection, scene) {
    const run = result.scenes;
    if (!run || run.scenes.length <= 1)
        return canvasOutputCatalog(result, inventory, inspection);
    const ordered = [...inventory].sort((a, b) => a.order - b.order);
    const scenes = Object.freeze(run.scenes.map((entry) => Object.freeze({
        key: entry.key,
        title: entry.title,
        available: entry.outputs.some((output) => output.items.length > 0) ||
            (run.running === entry.key && result.preview !== null),
    })));
    const chosen = run.scenes.find((entry) => entry.key === scene);
    if (chosen) {
        const preview = run.running === chosen.key ? result.preview : null;
        const presentedRun = latestRun(chosen.outputs, preview?.runOrder ?? 0);
        return catalog(result, inventory, sourceEntries(result, chosen.outputs, preview, presentedRun, ordered, inspection), scenes, false);
    }
    const entries = [];
    for (const entry of run.scenes) {
        const outputs = new Map(entry.outputs.map((output) => [output.source.id, output]));
        const presentedRun = latestRun(entry.outputs, 0);
        const representative = ordered
            .map((source) => ({
            source,
            item: outputs.get(source.id)?.items.find((item) => item.provenance.runOrder === presentedRun) ??
                null,
        }))
            .find((candidate) => candidate.item !== null);
        const preview = run.running === entry.key && !representative ? result.preview : null;
        const source = representative?.source ?? ordered.find((candidate) => candidate.id === preview?.sourceId);
        if (!source || (!representative?.item && !preview))
            continue;
        entries.push(Object.freeze({
            source,
            items: Object.freeze(representative?.item ? [representative.item] : []),
            preview,
            scene: entry.key,
            summary: Object.freeze({
                id: source.id,
                label: entry.title,
                breadcrumb: Object.freeze([]),
                batchCount: 1,
                preview: Boolean(preview),
                previousRun: false,
            }),
        }));
    }
    return catalog(result, inventory, Object.freeze(entries), scenes, true);
}
/** Read the newest execution run among final outputs, starting from a known floor. */
function latestRun(outputs, floor) {
    return outputs.reduce((latest, output) => output.items.reduce((run, item) => (item.file.type === 'input' ? run : Math.max(run, item.provenance.runOrder)), latest), floor);
}
/** Project each inventory source's presented outputs and live preview, in source order. */
function sourceEntries(result, sourceOutputs, livePreview, presentedRun, inventory, inspection) {
    const outputs = new Map(sourceOutputs.map((output) => [output.source.id, output]));
    const entries = [];
    for (const source of [...inventory].sort((a, b) => a.order - b.order)) {
        const output = outputs.get(source.id);
        const input = Boolean(output?.items.length && output.items.every((item) => item.file.type === 'input'));
        const items = output?.items.filter((item) => (input && presentedRun === 0) ||
            item.provenance.runOrder === presentedRun ||
            source.id === inspection?.sourceId) ?? [];
        const preview = livePreview?.sourceId === source.id ? livePreview : null;
        if (!preview && !items.length)
            continue;
        entries.push(Object.freeze({
            source,
            items: Object.freeze(items),
            preview,
            summary: Object.freeze({
                id: source.id,
                label: source.label,
                breadcrumb: source.breadcrumb,
                batchCount: Math.max(preview ? 1 : 0, items.length ? (output?.batchCount ?? 0) : 0),
                preview: Boolean(preview),
                previousRun: Boolean(items.some((item) => item.provenance.runOrder < result.runOrder)),
            }),
        }));
    }
    return qualifiedCaptions(entries);
}
/** Assemble one immutable catalog snapshot. */
function catalog(result, inventory, entries, scenes, sceneOverview) {
    return Object.freeze({
        sessionId: canvasSessionId(result.context.sessionId),
        generation: result.context.generation,
        sources: inventory,
        entries: Object.freeze([...entries]),
        result,
        ...(scenes ? { scenes, sceneOverview: Boolean(sceneOverview) } : {}),
    });
}
/** Disambiguate collisions within the output catalog without prefixing every authored source caption. */
function qualifiedCaptions(entries) {
    const labels = new Map();
    for (const entry of entries)
        labels.set(entry.summary.label, (labels.get(entry.summary.label) ?? 0) + 1);
    const occurrences = new Map();
    const captions = new Map();
    for (const entry of entries) {
        const caption = [...entry.source.breadcrumb, entry.source.label].join(' / ');
        captions.set(caption, (captions.get(caption) ?? 0) + 1);
    }
    return entries.map((entry) => {
        if ((labels.get(entry.summary.label) ?? 0) <= 1)
            return entry;
        const label = [...entry.source.breadcrumb, entry.source.label].join(' / ');
        const occurrence = (occurrences.get(label) ?? 0) + 1;
        occurrences.set(label, occurrence);
        const repeated = (captions.get(label) ?? 0) > 1;
        return Object.freeze({
            ...entry,
            summary: Object.freeze({
                ...entry.summary,
                label: repeated ? `${label} (${occurrence})` : label,
            }),
        });
    });
}
