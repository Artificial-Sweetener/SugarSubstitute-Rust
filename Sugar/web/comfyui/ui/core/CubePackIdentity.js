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
/** Resolve Cube catalog provenance into one stable pack presentation identity. */
import { isRecord } from '../types/common.js';
/** Resolve current picker, browser, and legacy source shapes through one policy owner. */
export function resolveCubePackIdentity(value) {
    const entry = isRecord(value) ? value : {};
    const source = isRecord(entry.source) ? entry.source : {};
    const owner = readText(entry.owner) || readText(source.owner);
    const repo = readText(entry.repo) || readText(source.repo);
    if (owner && repo)
        return githubIdentity(owner, repo, 'github');
    const repoRef = readText(source.repoRef) || readText(source.repo_ref);
    const [repoOwner = '', repoName = ''] = repoRef.split('/', 2).map((part) => part.trim());
    if (repoOwner && repoName)
        return githubIdentity(repoOwner, repoName, 'github');
    const namespace = readText(entry.namespace) || readText(source.namespace);
    const sourceKind = readText(source.kind) || readText(source.type) || readText(source.sourceKind);
    if (sourceKind === 'local' || namespace) {
        const localNamespace = namespace || 'local';
        return {
            key: `local:${localNamespace.toLowerCase()}`,
            label: 'local',
            authorLabel: namespace,
        };
    }
    const author = readText(entry.author);
    const [legacyOwner = '', legacyRepo = ''] = author.split('/', 2).map((part) => part.trim());
    if (legacyOwner && legacyRepo)
        return githubIdentity(legacyOwner, legacyRepo, 'legacy');
    const label = author || 'Unknown';
    return { key: `legacy:${label.toLowerCase()}`, label, authorLabel: '' };
}
/** Build one normalized hosted-pack identity. */
function githubIdentity(owner, repo, prefix) {
    return {
        key: `${prefix}:${owner.toLowerCase()}/${repo.toLowerCase()}`,
        label: repo,
        authorLabel: owner,
    };
}
/** Normalize optional external text without accepting coercion. */
function readText(value) {
    return typeof value === 'string' ? value.trim() : '';
}
