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
/** Generate bounded language-neutral contracts and validation from one closed wire schema. */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { format } from 'prettier';
type Schema = Record<string, unknown>;

/** Narrow authored schema nodes before producing executable bindings. */
function record(value: unknown): Schema {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid contract schema node.');
  return value as Schema;
}

/** Translate the deliberately limited schema vocabulary into one TypeScript type. */
function binding(value: unknown): string {
  const node = record(value);
  if (typeof node.$ref === 'string') return reference(node.$ref);
  if (node.const !== undefined) return JSON.stringify(node.const);
  if (Array.isArray(node.enum))
    return node.enum.map((item: unknown) => JSON.stringify(item)).join(' | ');
  if (Array.isArray(node.oneOf)) return node.oneOf.map(binding).join(' | ');
  if (node.type === 'boolean' || node.type === 'string') return node.type;
  if (node.type === 'integer' || node.type === 'number') return 'number';
  if (node.type === 'array') return `readonly (${binding(node.items)})[]`;
  if (node.type === 'object') {
    const properties = record(node.properties);
    if (
      node.additionalProperties !== false ||
      JSON.stringify(node.required) !== JSON.stringify(Object.keys(properties))
    )
      throw new Error('Contract objects must be closed with every property required.');
    return `{ ${Object.entries(properties)
      .map(([key, property]) => `readonly ${key}: ${binding(property)};`)
      .join(' ')} }`;
  }
  throw new Error('Unsupported contract schema vocabulary.');
}

/** Resolve only local, identifier-safe schema definitions. */
function reference(value: string): string {
  const match = /^#\/\$defs\/([A-Za-z]+)$/.exec(value);
  if (!match?.[1]) throw new Error('Unsupported contract schema reference.');
  return match[1];
}

/** Emit fail-closed validation directly from the same closed wire schema. */
function predicate(value: unknown, expression: string): string {
  const node = record(value);
  if (typeof node.$ref === 'string') return `is${reference(node.$ref)}(${expression})`;
  if (node.const !== undefined) return `${expression} === ${JSON.stringify(node.const)}`;
  if (Array.isArray(node.enum))
    return `(${node.enum.map((item: unknown) => `${expression} === ${JSON.stringify(item)}`).join(' || ')})`;
  if (Array.isArray(node.oneOf))
    return `(${node.oneOf.map((item: unknown) => predicate(item, expression)).join(' || ')})`;
  if (node.type === 'boolean') return `typeof ${expression} === 'boolean'`;
  if (node.type === 'array') {
    if (typeof node.maxItems !== 'number') throw new Error('Contract arrays require a bound.');
    return `Array.isArray(${expression}) && ${expression}.length <= ${node.maxItems} && ${expression}.every((item: unknown) => (${predicate(node.items, 'item')}))`;
  }
  if (node.type === 'integer' || node.type === 'number') {
    if (typeof node.minimum !== 'number' || typeof node.maximum !== 'number')
      throw new Error('Contract numbers require lower and upper bounds.');
    const admission = node.type === 'integer' ? 'Number.isSafeInteger' : 'Number.isFinite';
    return `typeof ${expression} === 'number' && ${admission}(${expression}) && ${expression} >= ${node.minimum} && ${expression} <= ${node.maximum}`;
  }
  if (node.type === 'string') {
    if (typeof node.maxLength !== 'number') throw new Error('Contract strings require a bound.');
    return `typeof ${expression} === 'string' && ${expression}.length <= ${node.maxLength}${typeof node.pattern === 'string' ? ` && new RegExp(${JSON.stringify(node.pattern)}).test(${expression})` : ''}`;
  }
  if (node.type === 'object') {
    const properties = Object.entries(record(node.properties));
    return `isRecord(${expression}) && Object.keys(${expression}).length === ${properties.length} && ${properties.map(([key, property]) => `Object.hasOwn(${expression}, ${JSON.stringify(key)}) && (${predicate(property, `${expression}[${JSON.stringify(key)}]`)})`).join(' && ')}`;
  }
  throw new Error('Unsupported contract validation vocabulary.');
}

/** Identify generated ownership without duplicating schema interpretation. */
export interface ContractOutput {
  readonly union: string;
  readonly schema: string;
  readonly generator: string;
  readonly types: string;
  readonly validation: string;
  readonly importPath: string;
  readonly description: string;
}
/** Generate deterministic values and guards through the repository's shared schema vocabulary. */
export async function contractBindings(
  schemaText: string,
  options: ContractOutput,
): Promise<Readonly<Record<string, string>>> {
  const schema = record(JSON.parse(schemaText));
  const definitions = Object.entries(record(schema.$defs));
  const header = readFileSync(fileURLToPath(import.meta.url), 'utf8').split('/**')[0] ?? '';
  const types = definitions
    .map(
      ([name, node]) =>
        `/** Describe the schema-defined ${name} wire value. */\nexport type ${name} = ${binding(node)};`,
    )
    .join('\n');
  const guards = [...definitions, [options.union, schema] as const]
    .map(
      ([name, node]) =>
        `/** Validate a bounded ${name} payload before dispatch. */\nexport function is${name}(value: unknown): value is Contract.${name} { return ${predicate(node, 'value')}; }`,
    )
    .join('\n');
  const sources = {
    [options.types]: `${header}/** Generated from ${options.schema} by ${options.generator}. */\n${types}\n/** Describe ${options.description}. */\nexport type ${options.union} = ${binding(schema)};`,
    [options.validation]: `${header}/** Generated protocol validation; edit the authoritative ${options.schema}. */\nimport type * as Contract from '${options.importPath}';\n/** Narrow external records without allowing arrays. */\nfunction isRecord(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }\n${guards}`,
  };
  const result: Record<string, string> = {};
  for (const [file, source] of Object.entries(sources))
    result[file] = await format(source, {
      parser: 'typescript',
      singleQuote: true,
      printWidth: 100,
    });
  return result;
}
