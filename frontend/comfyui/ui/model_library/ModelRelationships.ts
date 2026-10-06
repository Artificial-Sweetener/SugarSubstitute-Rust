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
/** Validate portable provider identities without duplicating backend compatibility policy. */
import { isRecord } from '../types/common.js';

export interface ModelFamily {
  readonly key: string;
  readonly name: string;
  readonly parent: string;
  readonly architecture: string;
  readonly bases: readonly string[];
}

export interface ModelRelationshipCatalog {
  readonly revision: string;
  readonly families: readonly ModelFamily[];
}

/** Keep older public catalogs usable while bounding newly supplied relationship records. */
export function decodeModelRelationships(value: unknown): ModelRelationshipCatalog | null {
  if (value === undefined || value === null) return null;
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    typeof value.revision !== 'string' ||
    !Array.isArray(value.families) ||
    value.families.length > 2048
  )
    throw new Error('Invalid model relationship catalog.');
  const families = value.families.map((item: unknown) => {
    if (!isRecord(item) || !Array.isArray(item.bases) || item.bases.length > 2048)
      throw new Error('Invalid model family.');
    return {
      key: text(item.key),
      name: text(item.name),
      parent: text(item.parent),
      architecture: text(item.architecture),
      bases: item.bases.map(text),
    };
  });
  if (new Set(families.map((family) => family.key)).size !== families.length)
    throw new Error('Duplicate model family.');
  return { revision: value.revision, families };
}

/** Resolve exact provider names, stable keys and declared base labels without filename heuristics. */
export function resolveModelFamily(
  catalog: ModelRelationshipCatalog | null | undefined,
  label: string,
): ModelFamily | null {
  const normalized = label.trim().toLowerCase();
  return (
    catalog?.families.find((family) =>
      [family.key, family.name, ...family.bases].some((name) => name.toLowerCase() === normalized),
    ) ?? null
  );
}

/** Narrow provider labels before they cross into consumers. */
function text(value: unknown): string {
  if (typeof value !== 'string' || value.length > 512)
    throw new Error('Invalid model family label.');
  return value;
}
