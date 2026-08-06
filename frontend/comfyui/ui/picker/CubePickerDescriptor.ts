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
/** Validate host-neutral Cube picker descriptors at the network boundary. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';

const PICKER_SCHEMA_VERSION = 1;
const PICKER_KEY_PATTERN = /^[a-f0-9]{64}$/u;

export interface CubePickerBoundary {
  id: string;
  name: string;
  label: string;
  type: string;
}

export interface CubePickerSource extends UnknownRecord {
  kind?: string;
  repoRef?: string;
  path?: string;
}

export interface CubePickerDescriptor {
  key: string;
  cubeId: string;
  version: string;
  displayName: string;
  description: string;
  searchTerms: string[];
  targetModel: string;
  supportedModels: string[];
  requiredCustomNodes: string[];
  source: CubePickerSource;
  inputs: CubePickerBoundary[];
  outputs: CubePickerBoundary[];
}

export interface CubePickerCatalogError {
  cubeId: string;
  message: string;
}

export interface CubePickerCatalog {
  schemaVersion: typeof PICKER_SCHEMA_VERSION;
  catalogRevision: string;
  entries: CubePickerDescriptor[];
  errors: CubePickerCatalogError[];
}

/** Parse one complete picker catalog or reject an incompatible response. */
export function readCubePickerCatalog(value: unknown): CubePickerCatalog | null {
  if (!isRecord(value) || value.schemaVersion !== PICKER_SCHEMA_VERSION) return null;
  if (typeof value.catalogRevision !== 'string' || !value.catalogRevision.trim()) return null;
  if (!Array.isArray(value.entries) || !Array.isArray(value.errors)) return null;
  const entries = value.entries.map(readCubePickerDescriptor);
  const errors = value.errors.map(readCatalogError);
  if (entries.some((entry) => entry === null) || errors.some((error) => error === null)) {
    return null;
  }
  const validEntries = entries.filter((entry): entry is CubePickerDescriptor => entry !== null);
  if (hasDuplicateDescriptorIdentity(validEntries)) return null;
  return {
    schemaVersion: PICKER_SCHEMA_VERSION,
    catalogRevision: value.catalogRevision.trim(),
    entries: validEntries,
    errors: errors.filter((error): error is CubePickerCatalogError => error !== null),
  };
}

/** Reject catalog-level type or Cube identity collisions before reconciliation. */
function hasDuplicateDescriptorIdentity(entries: readonly CubePickerDescriptor[]): boolean {
  const keys = new Set<string>();
  const cubeIds = new Set<string>();
  for (const entry of entries) {
    if (keys.has(entry.key) || cubeIds.has(entry.cubeId)) return true;
    keys.add(entry.key);
    cubeIds.add(entry.cubeId);
  }
  return false;
}

/** Parse one descriptor with unique canonical boundary identities. */
export function readCubePickerDescriptor(value: unknown): CubePickerDescriptor | null {
  if (!isRecord(value)) return null;
  const key = readRequiredString(value.key);
  const cubeId = readRequiredString(value.cubeId);
  const displayName = readRequiredString(value.displayName);
  if (!PICKER_KEY_PATTERN.test(key) || !cubeId || !displayName) return null;
  const inputs = readBoundaryList(value.inputs);
  const outputs = readBoundaryList(value.outputs);
  const searchTerms = readStringList(value.searchTerms);
  const supportedModels = readStringList(value.supportedModels);
  const requiredCustomNodes = readStringList(value.requiredCustomNodes);
  if (!inputs || !outputs || !searchTerms || !supportedModels || !requiredCustomNodes) return null;
  const source = isRecord(value.source) ? { ...value.source } : null;
  if (!source) return null;
  return {
    key,
    cubeId,
    version: readOptionalString(value.version),
    displayName,
    description: readOptionalString(value.description),
    searchTerms,
    targetModel: readOptionalString(value.targetModel),
    supportedModels,
    requiredCustomNodes,
    source,
    inputs,
    outputs,
  };
}

/** Parse boundary arrays without permitting identity collisions. */
function readBoundaryList(value: unknown): CubePickerBoundary[] | null {
  if (!Array.isArray(value)) return null;
  const boundaries = value.map(readBoundary);
  if (boundaries.some((boundary) => boundary === null)) return null;
  const valid = boundaries.filter((boundary): boundary is CubePickerBoundary => boundary !== null);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const boundary of valid) {
    if (ids.has(boundary.id) || names.has(boundary.name)) return null;
    ids.add(boundary.id);
    names.add(boundary.name);
  }
  return valid;
}

/** Parse one canonical public boundary. */
function readBoundary(value: unknown): CubePickerBoundary | null {
  if (!isRecord(value)) return null;
  const id = readRequiredString(value.id);
  const name = readRequiredString(value.name);
  const label = readRequiredString(value.label);
  const type = readRequiredString(value.type);
  return id && name && label && type ? { id, name, label, type } : null;
}

/** Parse a string array without silently discarding dynamic values. */
function readStringList(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) return null;
  return value.map((item) => item.trim()).filter(Boolean);
}

/** Parse one backend projection error. */
function readCatalogError(value: unknown): CubePickerCatalogError | null {
  if (!isRecord(value)) return null;
  const cubeId = readOptionalString(value.cubeId);
  const message = readRequiredString(value.message);
  return message ? { cubeId, message } : null;
}

/** Normalize one required string. */
function readRequiredString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Normalize one optional string. */
function readOptionalString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
