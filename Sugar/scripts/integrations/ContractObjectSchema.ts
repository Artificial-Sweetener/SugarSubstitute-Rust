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
/** Admit the closed object vocabulary shared by generated protocol bindings. */

type Schema = Record<string, unknown>;

/** Describe one declared member and whether the wire value may omit it. */
export interface ContractObjectProperty {
  readonly key: string;
  readonly value: unknown;
  readonly optional: boolean;
}

/**
 * Read a closed object whose `required` list names declared members in declaration order.
 *
 * Omitted members are optional; an absent member and an explicit `null` remain distinct, so
 * generated validators reject `null` for an optional member unless its own schema admits it.
 */
export function contractObjectProperties(node: Schema): readonly ContractObjectProperty[] {
  const properties = node.properties;
  const required = node.required;
  if (
    node.type !== 'object' ||
    node.additionalProperties !== false ||
    !properties ||
    typeof properties !== 'object' ||
    Array.isArray(properties) ||
    !Array.isArray(required) ||
    !required.every((key: unknown): key is string => typeof key === 'string')
  )
    throw new Error('Contract objects must be closed with an explicit required list.');
  const entries = Object.entries(properties as Schema);
  const declared = entries.map(([key]) => key);
  const ordered = declared.filter((key) => required.includes(key));
  if (JSON.stringify(ordered) !== JSON.stringify(required))
    throw new Error('Contract required members must be unique declared members in order.');
  return entries.map(([key, value]) => ({ key, value, optional: !required.includes(key) }));
}
