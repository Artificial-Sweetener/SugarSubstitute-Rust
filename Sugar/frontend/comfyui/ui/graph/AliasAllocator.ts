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
 * Own graph-wide SugarCubes instance alias allocation.
 */

import { getGroupSugarcubes } from './GroupMetadata.js';
import type { ComfyGroup } from '../types/graph.js';

interface MatchedInstance {
  instanceId?: unknown;
  instanceAlias?: unknown;
  defaultAlias?: unknown;
  cubeId?: unknown;
}

export interface InstanceMatch {
  group?: ComfyGroup | null;
  instance?: MatchedInstance | null;
  order?: number;
}

interface OrderedInstanceMatch extends InstanceMatch {
  instance: MatchedInstance;
  order: number;
}

interface AliasAllocationOptions {
  ignoreGroup?: (group: ComfyGroup | null | undefined, metadata: unknown) => boolean;
}

function readName(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function normalizeInstanceAlias(value: unknown): string {
  return readName(value).toLowerCase();
}

function readMatchInstanceId(match: InstanceMatch): string {
  return (
    readName(match?.instance?.instanceId) || readName(getGroupSugarcubes(match?.group)?.instance_id)
  );
}

function isImportedGroup(group: ComfyGroup | null | undefined): boolean {
  return Boolean(group?.__sugarcubes_imported);
}

function isExistingMatch(match: InstanceMatch): boolean {
  const metadata = getGroupSugarcubes(match?.group);
  const instanceId = readMatchInstanceId(match);
  const metadataInstanceId = readName(metadata?.instance_id);
  return Boolean(
    match?.group &&
      metadata?.managed &&
      instanceId &&
      metadataInstanceId &&
      instanceId === metadataInstanceId &&
      !isImportedGroup(match.group),
  );
}

function readInstanceAliasSeed(match: InstanceMatch): string {
  const metadata = getGroupSugarcubes(match?.group);
  const metadataAlias = readName(metadata?.instance_alias);
  const instanceAlias = readName(match?.instance?.instanceAlias);
  const defaultAlias = readName(metadata?.default_alias) || readName(match?.instance?.defaultAlias);
  if (isExistingMatch(match)) {
    return (
      metadataAlias ||
      instanceAlias ||
      defaultAlias ||
      readName(match?.instance?.cubeId) ||
      'SugarCube'
    );
  }
  return (
    instanceAlias ||
    metadataAlias ||
    defaultAlias ||
    readName(match?.instance?.cubeId) ||
    'SugarCube'
  );
}

function allocateInstanceAlias(seed: unknown, taken: ReadonlySet<string>): string {
  const base = readName(seed) || 'SugarCube';
  const baseKey = normalizeInstanceAlias(base);
  if (!baseKey || !taken.has(baseKey)) {
    return base;
  }
  let index = 2;
  while (index < 1000) {
    const next = `${base} ${index}`;
    if (!taken.has(normalizeInstanceAlias(next))) {
      return next;
    }
    index += 1;
  }
  return `${base} ${Date.now()}`;
}

/**
 * Allocate instance aliases for all matched live instances in a deterministic graph-wide pass.
 */
export function allocateGraphInstanceAliases(
  instanceMatches: readonly InstanceMatch[] = [],
  options: AliasAllocationOptions = {},
): Map<string, string> {
  const ignoreGroup = typeof options.ignoreGroup === 'function' ? options.ignoreGroup : null;
  const matches: OrderedInstanceMatch[] = Array.isArray(instanceMatches)
    ? instanceMatches
        .filter((match): match is InstanceMatch & { instance: MatchedInstance } =>
          Boolean(match.instance),
        )
        .map((match, index) => ({
          ...match,
          order:
            typeof match.order === 'number' && Number.isInteger(match.order) ? match.order : index,
        }))
    : [];
  const instanceAliases = new Map<string, string>();
  const taken = new Set<string>();
  const ordered = matches.sort((left, right) => {
    const leftExisting = isExistingMatch(left);
    const rightExisting = isExistingMatch(right);
    if (leftExisting !== rightExisting) {
      return leftExisting ? -1 : 1;
    }
    return left.order - right.order;
  });

  for (const match of ordered) {
    const metadata = getGroupSugarcubes(match.group);
    if (ignoreGroup?.(match.group, metadata)) {
      continue;
    }
    const instanceId = readMatchInstanceId(match);
    if (!instanceId) {
      continue;
    }
    const instanceAlias = allocateInstanceAlias(readInstanceAliasSeed(match), taken);
    instanceAliases.set(instanceId, instanceAlias);
    taken.add(normalizeInstanceAlias(instanceAlias));
  }

  return instanceAliases;
}
