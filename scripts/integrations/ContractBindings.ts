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
import { contractArraySchema } from './ContractArraySchema.js';
import { contractObjectProperties } from './ContractObjectSchema.js';
import {
  contractValidationModules,
  type ContractValidationModule,
} from './ContractValidationModules.js';
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
  if (node.type === 'array') {
    const array = contractArraySchema(node);
    return `readonly (${binding(array.items)})[]`;
  }
  if (typeof node.$ref === 'string') return reference(node.$ref);
  if (node.const !== undefined) return JSON.stringify(node.const);
  if (Array.isArray(node.enum))
    return node.enum.map((item: unknown) => JSON.stringify(item)).join(' | ');
  if (Array.isArray(node.oneOf)) return node.oneOf.map(binding).join(' | ');
  if (node.type === 'boolean' || node.type === 'string') return node.type;
  if (node.type === 'integer' || node.type === 'number') return 'number';
  if (node.type === 'object')
    return `{ ${contractObjectProperties(node)
      .map(
        ({ key, value, optional }) => `readonly ${key}${optional ? '?' : ''}: ${binding(value)};`,
      )
      .join(' ')} }`;
  throw new Error('Unsupported contract schema vocabulary.');
}

/** Resolve only local, identifier-safe schema definitions. */
function reference(value: string): string {
  const match = /^#\/\$defs\/([A-Za-z]+)$/.exec(value);
  if (!match?.[1]) throw new Error('Unsupported contract schema reference.');
  return match[1];
}

/** Emit fail-closed validation directly from the same closed wire schema. */
function predicate(value: unknown, expression: string, references: Set<string>): string {
  const node = record(value);
  if (node.type === 'array') {
    const array = contractArraySchema(node);
    const minimum = array.minimum ? `${expression}.length >= ${array.minimum} && ` : '';
    return `Array.isArray(${expression}) && ${minimum}${expression}.length <= ${array.maximum} && ${expression}.every((item: unknown) => (${predicate(array.items, 'item', references)}))`;
  }
  if (typeof node.$ref === 'string') {
    const name = reference(node.$ref);
    references.add(name);
    return `is${name}(${expression})`;
  }
  if (node.const !== undefined) return `${expression} === ${JSON.stringify(node.const)}`;
  if (Array.isArray(node.enum))
    return `(${node.enum.map((item: unknown) => `${expression} === ${JSON.stringify(item)}`).join(' || ')})`;
  if (Array.isArray(node.oneOf))
    return `(${node.oneOf.map((item: unknown) => predicate(item, expression, references)).join(' || ')})`;
  if (node.type === 'boolean') return `typeof ${expression} === 'boolean'`;
  if (node.type === 'integer' || node.type === 'number') {
    const minimum = typeof node.minimum === 'number' ? ` && ${expression} >= ${node.minimum}` : '';
    const exclusiveMinimum =
      typeof node.exclusiveMinimum === 'number'
        ? ` && ${expression} > ${node.exclusiveMinimum}`
        : '';
    if ((!minimum && !exclusiveMinimum) || typeof node.maximum !== 'number')
      throw new Error('Contract numbers require lower and upper bounds.');
    const admission = node.type === 'integer' ? 'Number.isSafeInteger' : 'Number.isFinite';
    return `typeof ${expression} === 'number' && ${admission}(${expression})${minimum}${exclusiveMinimum} && ${expression} <= ${node.maximum}`;
  }
  if (node.type === 'string') {
    if (typeof node.maxLength !== 'number') throw new Error('Contract strings require a bound.');
    if (node['x-length-unit'] !== undefined && node['x-length-unit'] !== 'unicode-codepoints')
      throw new Error('Unsupported contract string length unit.');
    if (node['x-length-unit'] === 'unicode-codepoints' && !Number.isSafeInteger(node.maxLength))
      throw new Error('Unicode contract strings require a finite integer bound.');
    const length =
      node['x-length-unit'] === 'unicode-codepoints'
        ? `${expression}.length <= ${node.maxLength * 2} && Array.from(${expression}).length <= ${node.maxLength}`
        : `${expression}.length <= ${node.maxLength}`;
    return `typeof ${expression} === 'string' && ${length}${typeof node.pattern === 'string' ? ` && new RegExp(${JSON.stringify(node.pattern)}).test(${expression})` : ''}`;
  }
  if (node.type === 'object') {
    const properties = contractObjectProperties(node);
    const member = (key: string) => `Object.hasOwn(${expression}, ${JSON.stringify(key)})`;
    const required = properties.filter(({ optional }) => !optional).length;
    // Counting only declared members keeps the object closed while optional members vary.
    const count = [
      String(required),
      ...properties.filter(({ optional }) => optional).map(({ key }) => `(${member(key)} ? 1 : 0)`),
    ].join(' + ');
    return `isRecord(${expression}) && Object.keys(${expression}).length === ${count} && ${properties
      .map(({ key, value, optional }) => {
        const valid = predicate(value, `${expression}[${JSON.stringify(key)}]`, references);
        return optional ? `(!${member(key)} || (${valid}))` : `${member(key)} && (${valid})`;
      })
      .join(' && ')}`;
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
  /** Share a record guard when related generated schemas live in separate modules. */
  readonly recordImport?: string;
  /** Place related validators in explicitly owned sibling modules; retain public guard exports. */
  readonly validationModules?: readonly ContractValidationModule[];
  /** Import one separately generated union while keeping a single public root guard. */
  readonly external?: {
    readonly name: string;
    readonly typesImport: string;
    readonly validationImport: string;
  };
}
/** Generate deterministic values and guards through the repository's shared schema vocabulary. */
export async function contractBindings(
  schemaText: string,
  options: ContractOutput,
): Promise<Readonly<Record<string, string>>> {
  const schema = record(JSON.parse(schemaText));
  const definitions = Object.entries(record(schema.$defs));
  const header = readFileSync(fileURLToPath(import.meta.url), 'utf8').split('/**')[0] ?? '';
  const external = options.external;
  if (
    external &&
    (!/^[A-Za-z]+$/.test(external.name) ||
      !/^\.\.?\/[A-Za-z0-9./-]+\.js$/.test(external.typesImport) ||
      !/^\.\.?\/[A-Za-z0-9./-]+\.js$/.test(external.validationImport))
  )
    throw new Error('Invalid external contract union.');
  const externalType = external
    ? `import type { ${external.name} } from '${external.typesImport}';\n`
    : '';
  const externalGuard = external
    ? `import { is${external.name} } from '${external.validationImport}';\n`
    : '';
  if (options.recordImport && !/^\.\.?\/[A-Za-z0-9./-]+\.js$/.test(options.recordImport))
    throw new Error('Invalid shared record guard.');
  const recordGuard = options.recordImport
    ? `import { isRecord } from '${options.recordImport}';\n`
    : `/** Narrow external records without allowing arrays. */\nfunction isRecord(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }\n`;
  const types = definitions
    .map(
      ([name, node]) =>
        `/** Describe the schema-defined ${name} wire value. */\nexport type ${name} = ${binding(node)};`,
    )
    .join('\n');
  const guards = [...definitions, [options.union, schema] as const].map(([name, node]) => {
    const references = new Set<string>();
    const body = predicate(node, 'value', references);
    return {
      name,
      references,
      source: `/** Validate a bounded ${name} payload before dispatch. */\nexport function is${name}(value: unknown): value is Contract.${name} { return ${body}; }`,
    };
  });
  const validationHeader = `${header}/** Generated protocol validation; edit the authoritative ${options.schema}. */\n`;
  const validators = options.validationModules
    ? contractValidationModules(guards, options.validationModules, {
        ...options,
        header: validationHeader,
        recordGuard,
        external: external ? { name: external.name, guard: externalGuard } : undefined,
      })
    : {
        [options.validation]: `${validationHeader}import type * as Contract from '${options.importPath}';\n${externalGuard}${recordGuard}${guards.map(({ source }) => source).join('\n')}`,
      };
  const sources = {
    [options.types]: `${header}/** Generated from ${options.schema} by ${options.generator}. */\n${externalType}${types}\n/** Describe ${options.description}. */\nexport type ${options.union} = ${binding(schema)};`,
    ...validators,
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
