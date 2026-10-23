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
/** Generate portable TypeScript values and bounded validators from the panel wire schema. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { format } from 'prettier';

type Schema = Record<string, unknown>;

/** Narrow authored schema nodes before producing executable bindings. */
function record(value: unknown): Schema {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid panel schema node.');
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
  if (node.type === 'integer') return 'number';
  if (node.type === 'object') {
    const properties = record(node.properties);
    if (
      node.additionalProperties !== false ||
      JSON.stringify(node.required) !== JSON.stringify(Object.keys(properties))
    )
      throw new Error('Panel objects must be closed with every property required.');
    return `{ ${Object.entries(properties)
      .map(([key, property]) => `readonly ${key}: ${binding(property)};`)
      .join(' ')} }`;
  }
  throw new Error('Unsupported panel schema vocabulary.');
}

/** Resolve only local, identifier-safe schema definitions. */
function reference(value: string): string {
  const match = /^#\/\$defs\/([A-Za-z]+)$/.exec(value);
  if (!match?.[1]) throw new Error('Unsupported panel schema reference.');
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
  if (node.type === 'integer') {
    if (typeof node.minimum !== 'number' || typeof node.maximum !== 'number')
      throw new Error('Panel integers require lower and upper bounds.');
    return `typeof ${expression} === 'number' && Number.isSafeInteger(${expression}) && ${expression} >= ${node.minimum} && ${expression} <= ${node.maximum}`;
  }
  if (node.type === 'string') {
    if (typeof node.maxLength !== 'number') throw new Error('Panel strings require a bound.');
    return `typeof ${expression} === 'string' && ${expression}.length <= ${node.maxLength}${typeof node.pattern === 'string' ? ` && new RegExp(${JSON.stringify(node.pattern)}).test(${expression})` : ''}`;
  }
  if (node.type === 'object') {
    const properties = Object.entries(record(node.properties));
    return `isRecord(${expression}) && Object.keys(${expression}).length === ${properties.length} && ${properties.map(([key, property]) => `Object.hasOwn(${expression}, ${JSON.stringify(key)}) && (${predicate(property, `${expression}[${JSON.stringify(key)}]`)})`).join(' && ')}`;
  }
  throw new Error('Unsupported panel validation vocabulary.');
}

/** Return deterministic contract and shared-validator sources for builds and conformance tests. */
export async function panelBindings(schemaText: string): Promise<Readonly<Record<string, string>>> {
  const schema = record(JSON.parse(schemaText));
  const definitions = Object.entries(record(schema.$defs));
  const header = readFileSync(fileURLToPath(import.meta.url), 'utf8').split('/**')[0] ?? '';
  const types = definitions
    .map(
      ([name, node]) =>
        `/** Describe the schema-defined ${name} wire value. */\nexport type ${name} = ${binding(node)};`,
    )
    .join('\n');
  const guards = [...definitions, ['PanelMessage', schema] as const]
    .map(
      ([name, node]) =>
        `/** Validate a bounded ${name} payload before dispatch. */\nexport function is${name}(value: unknown): value is Contract.${name} { return ${predicate(node, 'value')}; }`,
    )
    .join('\n');
  const sources = {
    'integrations/contracts/PanelProtocol.generated.ts': `${header}/** Generated from panel.schema.json by scripts/integrations/generate-panel-contract.ts. */\n${types}\n/** Describe every version-three embedded panel message. */\nexport type PanelMessage = ${binding(schema)};`,
    'integrations/shared/PanelValidation.generated.ts': `${header}/** Generated protocol validation; edit the authoritative panel.schema.json. */\nimport type * as Contract from '../contracts/PanelProtocol.generated.js';\n/** Narrow external records without allowing arrays. */\nfunction isRecord(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }\n${guards}`,
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

/** Generate or check bindings without modifying application or host state. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const sources = await panelBindings(
    readFileSync(path.join(root, 'integrations/contracts/panel.schema.json'), 'utf8'),
  );
  for (const [relative, source] of Object.entries(sources)) {
    const file = path.join(root, relative);
    if (process.argv.includes('--check')) {
      if (readFileSync(file, 'utf8').replaceAll('\r\n', '\n') !== source)
        throw new Error(`Regenerate ${relative}.`);
    } else writeFileSync(file, source);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
