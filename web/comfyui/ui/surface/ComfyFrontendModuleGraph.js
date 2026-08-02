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
/** Traverse Comfy's installed same-origin frontend module graph by capability. */
const ENTRY_PATTERN = /\/assets\/index-[^/?]+\.js(?:\?.*)?$/;
const RELATIVE_JAVASCRIPT_REFERENCE_PATTERN = /(["'`])(\.\/[^"'`?#]+\.js(?:\?[^"'`]*)?)\1/g;
const MAXIMUM_FETCHED_MODULE_COUNT = 128;
const MAXIMUM_MODULE_DEPTH = 4;
/** Find the active Comfy entry script without depending on a build hash. */
export function findComfyFrontendEntry(documentRef) {
    return (Array.from(documentRef.scripts)
        .map((script) => script.src)
        .find((source) => ENTRY_PATTERN.test(source)) ?? null);
}
/** Traverse installed modules until every requested semantic capability is found. */
export async function discoverComfyFrontendModules(documentRef, requirements, fetchText) {
    const entry = findComfyFrontendEntry(documentRef);
    if (!entry)
        throw new Error('Comfy entry module was not found.');
    const pending = [{ depth: 0, priority: Number.MAX_SAFE_INTEGER, url: entry }];
    const queued = new Set([entry]);
    const visited = new Set();
    const modules = new Map();
    while (pending.length > 0 && !hasEveryRequiredModule(requirements, modules)) {
        pending.sort(comparePendingModules);
        const candidate = pending.shift();
        if (!candidate || visited.has(candidate.url))
            continue;
        if (visited.size >= MAXIMUM_FETCHED_MODULE_COUNT) {
            throw new Error('Comfy frontend module discovery exceeded its bounded fetch count.');
        }
        visited.add(candidate.url);
        const source = await fetchText(candidate.url);
        const module = { url: candidate.url, source };
        for (const requirement of requirements) {
            if (!modules.has(requirement.key) && requirement.matches(module)) {
                modules.set(requirement.key, module);
            }
        }
        if (hasEveryRequiredModule(requirements, modules) || candidate.depth >= MAXIMUM_MODULE_DEPTH) {
            continue;
        }
        for (const url of discoverReferencedModuleUrls(source, candidate.url)) {
            if (visited.has(url) || queued.has(url))
                continue;
            queued.add(url);
            pending.push({
                depth: candidate.depth + 1,
                priority: modulePriority(url),
                url,
            });
        }
    }
    return { entry, modules };
}
/** Return whether discovery has satisfied every non-optional capability. */
function hasEveryRequiredModule(requirements, modules) {
    return requirements.every((requirement) => requirement.required === false || modules.has(requirement.key));
}
/** Resolve every quoted relative JavaScript dependency without executing module code. */
export function discoverReferencedModuleUrls(source, moduleUrl) {
    const origin = new URL(moduleUrl).origin;
    const urls = new Set();
    for (const match of source.matchAll(RELATIVE_JAVASCRIPT_REFERENCE_PATTERN)) {
        const reference = match[2];
        if (!reference)
            continue;
        const resolved = new URL(reference, moduleUrl);
        if (resolved.origin === origin)
            urls.add(resolved.href);
    }
    return [...urls];
}
/** Prefer known composition roots while retaining exhaustive bounded traversal as fallback. */
function modulePriority(url) {
    const filename = new URL(url).pathname.split('/').pop() ?? '';
    if (/GraphView-/i.test(filename))
        return 500;
    if (/settingStore-|dialogService-/i.test(filename))
        return 400;
    if (/vendor-vue-core-|vendor-primevue-/i.test(filename))
        return 300;
    if (/main-/i.test(filename))
        return 200;
    return 0;
}
/** Sort highest-value modules first while retaining stable shallow traversal. */
function comparePendingModules(left, right) {
    return (right.priority - left.priority || left.depth - right.depth || left.url.localeCompare(right.url));
}
