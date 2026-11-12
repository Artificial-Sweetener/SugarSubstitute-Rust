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
/** Select the definitions one Rust module needs, inlining tagged union references. */
import { record, type Schema } from './RustContractSchema.js';

/** Resolve a local definition reference without admitting external documents. */
function referenceName(value: unknown): string | undefined {
  return typeof value === 'string' ? /^#\/\$defs\/([A-Za-z]+)$/.exec(value)?.[1] : undefined;
}

/** Read the exact constant a closed branch declares for one discriminator member. */
function tagOf(branch: Schema, key: string): unknown {
  const properties = branch.properties;
  if (!properties || typeof properties !== 'object' || Array.isArray(properties)) return undefined;
  const member = (properties as Schema)[key];
  return member && typeof member === 'object' && !Array.isArray(member)
    ? (member as Schema).const
    : undefined;
}

/**
 * Splice referenced unions and inline referenced closed objects when every resulting branch
 * shares one `type` or `kind` constant; otherwise retain the named branches unchanged.
 *
 * Rust's internally tagged enums cannot nest a second tagged union or a struct that repeats the
 * tag, so a wire-identical flat union keeps the decoder exact while the schema stays factored.
 */
function flattened(node: Schema, definitions: Schema, visiting: ReadonlySet<string>): Schema {
  if (!Array.isArray(node.oneOf)) return node;
  const branches = node.oneOf.flatMap((value: unknown): Schema[] => {
    const branch = record(value);
    const name = Object.keys(branch).join() === '$ref' ? referenceName(branch.$ref) : undefined;
    if (!name) return [branch];
    if (visiting.has(name)) throw new Error('Cyclic Rust contract union reference.');
    const target = record(definitions[name]);
    const inner = flattened(target, definitions, new Set([...visiting, name]));
    return Array.isArray(inner.oneOf) ? inner.oneOf.map(record) : [inner];
  });
  const tagged = ['type', 'kind'].some((key) =>
    branches.every((branch) => typeof tagOf(branch, key) === 'string'),
  );
  return tagged ? { oneOf: branches } : node;
}

/** Walk every reference reachable from a schema node, recording definition names. */
function collect(value: unknown, names: Set<string>, definitions: Schema): void {
  if (Array.isArray(value)) for (const item of value) collect(item, names, definitions);
  else if (value && typeof value === 'object')
    for (const [key, item] of Object.entries(value)) {
      const name = key === '$ref' ? referenceName(item) : undefined;
      if (key === '$ref' && !name) throw new Error('Unsupported Rust contract reference.');
      if (name && !names.has(name)) {
        if (!(name in definitions)) throw new Error(`Missing Rust contract definition: ${name}`);
        names.add(name);
        collect(
          flattened(record(definitions[name]), definitions, new Set([name])),
          names,
          definitions,
        );
      } else if (!name) collect(item, names, definitions);
    }
}

/**
 * Return the closure of the root definitions in schema order, with tagged union references
 * inlined. Definitions reachable only through an inlined reference are omitted.
 */
export function rustContractDefinitions(definitions: Schema, roots: readonly string[]): Schema {
  const names = new Set<string>();
  for (const root of roots) collect({ $ref: `#/$defs/${root}` }, names, definitions);
  return Object.fromEntries(
    Object.keys(definitions)
      .filter((name) => names.has(name))
      .map((name) => [name, flattened(record(definitions[name]), definitions, new Set([name]))]),
  );
}
