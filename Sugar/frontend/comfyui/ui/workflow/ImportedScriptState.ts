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
/** Read the editor state a project script's compiled workflow records beside its values. */
import { isRecord } from '../types/common.js';

/** One value a script gives to the same field of every Cube. */
export interface ImportedOverride {
  /** The input the override drives. */
  readonly key: string;
  readonly value: string | number | boolean;
}

/** The value one field of a following node keeps for itself, by relation field identity. */
export interface ImportedOwnValue {
  readonly field: string;
  readonly value: string | number | boolean;
}

/** What a script says about how its Cubes share values, as workflow relations identify it. */
export interface ImportedScriptRelations {
  readonly overrides: readonly ImportedOverride[];
  /** Overrides whose seed stays as it is when a run is queued. */
  readonly fixedSeedOverrides: readonly string[];
  /** Fields whose own seed stays as it is when a run is queued. */
  readonly fixedSeedFields: readonly string[];
  readonly ownValues: readonly ImportedOwnValue[];
}

const COMPOSITION_KEY = 'sugarcubes_composition';
/** Composition entries a script's workflow carries for its writer, which no page owner edits. */
const CARRIED = ['field_annotations', 'script_state'] as const;

/** Name one Cube input the way workflow relations identify a field. */
function fieldIdentity(endpoint: unknown): string | null {
  if (!isRecord(endpoint)) return null;
  const { instance_id: instance, node_symbol: symbol, input_name: input } = endpoint;
  return typeof instance === 'string' && typeof symbol === 'string' && typeof input === 'string'
    ? JSON.stringify([JSON.stringify(['cube', instance, symbol]), input])
    : null;
}

/** Tell whether a value is one a relation field can hold. */
function isValue(value: unknown): value is string | number | boolean {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean';
}

/** Read a script's composition, or `null` for a workflow that carries none. */
function composition(workflow: Record<string, unknown>): Record<string, unknown> | null {
  const held = isRecord(workflow.extra) ? workflow.extra[COMPOSITION_KEY] : null;
  return isRecord(held) ? held : null;
}

/** Read the overrides, own values and fixed seeds one compiled script records. */
export function readImportedScriptRelations(
  workflow: Record<string, unknown>,
): ImportedScriptRelations {
  const held = composition(workflow);
  const relations: unknown[] = Array.isArray(held?.value_relations) ? held.value_relations : [];
  const overrides: ImportedOverride[] = [];
  const ownValues: ImportedOwnValue[] = [];
  for (const relation of relations) {
    if (!isRecord(relation)) continue;
    if (relation.kind === 'global_override') {
      if (typeof relation.key === 'string' && isValue(relation.value))
        overrides.push({ key: relation.key, value: relation.value });
    } else if (relation.kind === 'field_link' && Array.isArray(relation.targets)) {
      for (const endpoint of relation.targets as unknown[]) {
        const field = fieldIdentity(endpoint);
        if (field && isValue(relation.own_value))
          ownValues.push({ field, value: relation.own_value });
      }
    }
  }
  const state = isRecord(held?.script_state) ? held.script_state : {};
  const fixedOverrides: unknown[] = Array.isArray(state.fixed_seed_overrides)
    ? state.fixed_seed_overrides
    : [];
  const fixedFields: unknown[] = Array.isArray(state.fixed_seed_fields)
    ? state.fixed_seed_fields
    : [];
  return {
    overrides,
    fixedSeedOverrides: fixedOverrides.filter((key) => typeof key === 'string'),
    fixedSeedFields: fixedFields.flatMap((endpoint) => fieldIdentity(endpoint) ?? []),
    ownValues,
  };
}

/** Name the Cube instances whose outputs a compiled script says are not saved. */
export function readUnsavedOutputInstances(workflow: Record<string, unknown>): readonly string[] {
  const nodes: unknown[] = Array.isArray(workflow.nodes) ? workflow.nodes : [];
  return nodes.flatMap((node) => {
    const properties = isRecord(node) && isRecord(node.properties) ? node.properties : null;
    const cube =
      properties && isRecord(properties.sugarcubes_cube) ? properties.sugarcubes_cube : null;
    return properties?.sugarcubes_save_outputs === false && typeof cube?.instance_id === 'string'
      ? [cube.instance_id]
      : [];
  });
}

/** Mark the named Cube instances of a workflow as not saving their outputs. */
export function markUnsavedOutputs(root: object, instanceIds: readonly string[]): void {
  if (!instanceIds.length) return;
  const wanted = new Set(instanceIds);
  const held: unknown = Reflect.get(root, '_nodes') ?? Reflect.get(root, 'nodes');
  for (const node of Array.isArray(held) ? (held as unknown[]) : []) {
    const properties = isRecord(node) && isRecord(node.properties) ? node.properties : null;
    const cube =
      properties && isRecord(properties.sugarcubes_cube) ? properties.sugarcubes_cube : null;
    if (properties && typeof cube?.instance_id === 'string' && wanted.has(cube.instance_id))
      properties.sugarcubes_save_outputs = false;
  }
}

/**
 * Keep, on the workflow being built, what a script's writer needs and no page owner edits:
 * the model and LoRA hashes beneath its fields and the script state nothing on the page shows.
 * The page's own records in the same metadata are left as they are.
 */
export function carryScriptComposition(root: object, workflow: Record<string, unknown>): void {
  const source = composition(workflow);
  if (!source) return;
  const carried = Object.fromEntries(
    CARRIED.flatMap((name) =>
      name in source ? [[name, JSON.parse(JSON.stringify(source[name])) as unknown]] : [],
    ),
  );
  if (!Object.keys(carried).length) return;
  const held: unknown = Reflect.get(root, 'extra');
  const extra = isRecord(held) ? held : {};
  const existing = isRecord(extra[COMPOSITION_KEY]) ? extra[COMPOSITION_KEY] : {};
  Reflect.set(root, 'extra', {
    ...extra,
    [COMPOSITION_KEY]: { ...existing, schema_version: 1, ...carried },
  });
}
