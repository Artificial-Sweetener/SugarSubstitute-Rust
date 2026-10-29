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
/** Place generated guards with their contract owner while retaining one public entry point. */
import { posix } from 'node:path';

/** Assign named schema definitions to one generated sibling owner. */
export interface ContractValidationModule {
  readonly file: string;
  readonly definitions: readonly string[];
}
/** Retain a compiled predicate and its schema-reference dependencies. */
export interface ContractGuard {
  readonly name: string;
  readonly source: string;
  readonly references: ReadonlySet<string>;
}

/** Reject overlapping owners and cyclic imports before emitting deterministic sibling modules. */
export function contractValidationModules(
  guards: readonly ContractGuard[],
  modules: readonly ContractValidationModule[],
  options: {
    readonly validation: string;
    readonly types: string;
    readonly union: string;
    readonly importPath: string;
    readonly header: string;
    readonly recordGuard: string;
    readonly external?: { readonly name: string; readonly guard: string } | undefined;
  },
): Record<string, string> {
  const owners = new Map(guards.map(({ name }) => [name, options.validation]));
  const files = new Map<string, readonly string[]>([[options.validation, []]]);
  for (const module of modules) {
    if (
      files.has(module.file) ||
      module.file === options.types ||
      posix.dirname(module.file) !== posix.dirname(options.validation) ||
      !/^[A-Za-z]+\.generated\.ts$/.test(posix.basename(module.file)) ||
      !module.definitions.length
    )
      throw new Error('Invalid generated contract validation module.');
    files.set(module.file, module.definitions);
    for (const name of module.definitions) {
      if (name === options.union || owners.get(name) !== options.validation)
        throw new Error('Contract validation definitions need one known owner.');
      owners.set(name, module.file);
    }
  }
  const dependencies = new Map<string, Map<string, Set<string>>>();
  for (const file of files.keys()) {
    const imports = new Map<string, Set<string>>();
    for (const guard of guards.filter(({ name }) => owners.get(name) === file)) {
      for (const name of guard.references) {
        const owner = owners.get(name);
        if (!owner) {
          if (name !== options.external?.name)
            throw new Error('Unknown contract validation reference.');
        } else if (owner !== file) {
          const names = imports.get(owner) ?? new Set<string>();
          names.add(name);
          imports.set(owner, names);
        }
      }
    }
    dependencies.set(file, imports);
  }
  const visited = new Set<string>();
  /** Refuse a generated module cycle rather than depending on initialization order. */
  function visit(file: string, active: ReadonlySet<string>): void {
    if (active.has(file)) throw new Error('Cyclic contract validation modules.');
    if (visited.has(file)) return;
    const imports = [
      ...(dependencies.get(file)?.keys() ?? []),
      ...(file === options.validation ? modules.map(({ file }) => file) : []),
    ];
    for (const dependency of imports) visit(dependency, new Set([...active, file]));
    visited.add(file);
  }
  for (const file of files.keys()) visit(file, new Set());
  const sources: Record<string, string> = {};
  for (const file of files.keys()) {
    const local = guards.filter(({ name }) => owners.get(name) === file);
    const imports = [...(dependencies.get(file) ?? [])]
      .map(
        ([dependency, names]) =>
          `import { ${[...names].map((name) => `is${name}`).join(', ')} } from './${posix.basename(dependency, '.ts')}.js';`,
      )
      .join('\n');
    const exports =
      file === options.validation
        ? modules
            .map(
              ({ file, definitions }) =>
                `export { ${definitions.map((name) => `is${name}`).join(', ')} } from './${posix.basename(file, '.ts')}.js';`,
            )
            .join('\n')
        : '';
    const external = options.external;
    const externalGuard =
      external && local.some(({ references }) => references.has(external.name))
        ? external.guard
        : '';
    sources[file] =
      `${options.header}import type * as Contract from '${options.importPath}';\n${imports}\n${externalGuard}${local.some(({ source }) => source.includes('isRecord(')) ? options.recordGuard : ''}${local.map(({ source }) => source).join('\n')}\n${exports}`;
  }
  return sources;
}
