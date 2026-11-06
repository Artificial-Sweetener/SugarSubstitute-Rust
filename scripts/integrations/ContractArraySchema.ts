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
/** Admit the bounded array vocabulary shared by generated protocol validators. */

type Schema = Record<string, unknown>;

/** Retain the item contract and its finite collection admission budget. */
export interface ContractArraySchema {
  readonly items: Schema;
  /** Zero unless the schema requires a nonempty collection. */
  readonly minimum: number;
  readonly maximum: number;
}

/** Reject tuple and dynamic item schemas before producing executable validators. */
function record(value: unknown): Schema {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid contract array item schema.');
  return value as Schema;
}

/** Admit only closed, nonempty string enums with unique literal tokens. */
export function contractArrayEnum(items: Schema): readonly string[] {
  const values = items.enum;
  if (
    Object.keys(items).some((key) => key !== 'enum') ||
    !Array.isArray(values) ||
    !values.length ||
    !values.every(
      (value: unknown): value is string => typeof value === 'string' && /^[a-z-]+$/.test(value),
    ) ||
    new Set(values).size !== values.length
  )
    throw new Error('Contract array items require a closed string enum.');
  return values;
}

/**
 * Preserve existing bounded item shapes while rejecting unknown array constraints.
 *
 * A declared `minItems` must be a positive integer no larger than `maxItems`; an explicit zero
 * restates the default and is refused so every lower bound in a schema is meaningful.
 */
export function contractArraySchema(node: Schema): ContractArraySchema {
  if (Object.keys(node).some((key) => !['type', 'items', 'minItems', 'maxItems'].includes(key)))
    throw new Error('Unsupported contract array schema keyword.');
  if (
    node.type !== 'array' ||
    typeof node.maxItems !== 'number' ||
    !Number.isSafeInteger(node.maxItems) ||
    node.maxItems < 0 ||
    node.maxItems > 65536
  )
    throw new Error('Contract arrays require a finite integer bound from 0 through 65536.');
  const minimum = node.minItems ?? 0;
  if (
    'minItems' in node &&
    (typeof minimum !== 'number' ||
      !Number.isSafeInteger(minimum) ||
      minimum < 1 ||
      minimum > node.maxItems)
  )
    throw new Error('Contract array minimums must be positive integers within the maximum.');
  if (typeof minimum !== 'number') throw new Error('Invalid contract array minimum.');
  const items = record(node.items);
  if ('enum' in items) contractArrayEnum(items);
  return { items, minimum, maximum: node.maxItems };
}
