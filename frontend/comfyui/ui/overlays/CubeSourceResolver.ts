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
/** Resolve browser and canonical cube identities into chrome badge sources. */

import { parseCanonicalCubeId } from '../core/CubeId.js';
import { isRecord, readString } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { BadgeSource, ChromeMetadata } from './CubeChromeOverlay.js';

export interface CubeSourceCatalog {
  getCubeById?(cubeId: string): unknown;
}

function buildGithubSource(author: unknown, pack: unknown): BadgeSource | null {
  const resolvedAuthor = typeof author === 'string' ? author.trim() : '';
  const resolvedPack = typeof pack === 'string' ? pack.trim() : '';
  if (!resolvedAuthor && !resolvedPack) {
    return null;
  }
  return { sourceKind: 'github', author: resolvedAuthor, pack: resolvedPack, namespace: '' };
}

function buildLocalSource(namespace: unknown): BadgeSource | null {
  const resolvedNamespace = typeof namespace === 'string' ? namespace.trim() : '';
  return resolvedNamespace
    ? { sourceKind: 'local', author: '', pack: '', namespace: resolvedNamespace }
    : null;
}

function sourceFromRepoRef(repoRef: unknown): BadgeSource | null {
  const resolvedRepoRef = typeof repoRef === 'string' ? repoRef.trim() : '';
  if (!resolvedRepoRef.includes('/')) {
    return null;
  }
  const [owner = '', repo = ''] = resolvedRepoRef.split('/', 2).map((part) => part.trim());
  return buildGithubSource(owner, repo);
}

function sourceFromLegacyAuthor(author: unknown): BadgeSource | null {
  const resolvedAuthor = typeof author === 'string' ? author.trim() : '';
  if (!resolvedAuthor.includes('/')) {
    return buildGithubSource(resolvedAuthor, '');
  }
  const [owner = '', repo = ''] = resolvedAuthor.split('/', 2).map((part) => part.trim());
  return buildGithubSource(owner, repo);
}

/** Resolve source metadata exposed by the cube catalog boundary. */
export function resolveCubeEntrySource(entry: unknown): BadgeSource | null {
  if (!isRecord(entry)) {
    return null;
  }
  const source: UnknownRecord = isRecord(entry.source) ? entry.source : {};
  const structuredGithub =
    buildGithubSource(entry.owner, entry.repo) || buildGithubSource(source.owner, source.repo);
  if (structuredGithub) {
    return structuredGithub;
  }
  const repoRefSource = sourceFromRepoRef(source.repo_ref);
  if (repoRefSource) {
    return repoRefSource;
  }
  const sourceKind = readString(source, 'type') || readString(source, 'sourceKind');
  if (sourceKind === 'local') {
    const localSource = buildLocalSource(source.namespace || entry.namespace);
    if (localSource) {
      return localSource;
    }
  }
  return sourceFromLegacyAuthor(entry.author);
}

/** Create the authoritative chrome source resolver for a cube catalog. */
export function createCubeSourceResolver(
  cubeBrowser: CubeSourceCatalog | null | undefined,
): (metadata: ChromeMetadata) => BadgeSource | null {
  return (metadata) => {
    const cubeId = typeof metadata.cube_id === 'string' ? metadata.cube_id.trim() : '';
    if (!cubeId) {
      return { sourceKind: '', author: '', pack: '', namespace: '' };
    }
    const catalogSource = resolveCubeEntrySource(cubeBrowser?.getCubeById?.(cubeId));
    if (catalogSource) {
      return catalogSource;
    }
    try {
      const parsed = parseCanonicalCubeId(cubeId);
      return parsed.sourceKind === 'github'
        ? buildGithubSource(parsed.owner, parsed.repo)
        : buildLocalSource(parsed.namespace);
    } catch {
      return { sourceKind: '', author: '', pack: '', namespace: '' };
    }
  };
}
