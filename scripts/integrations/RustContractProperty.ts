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
/** Compile one closed-schema member into a Rust field type and its exact bound checks. */
import { contractArrayEnum, contractArraySchema } from './ContractArraySchema.js';
import {
  keys,
  literal,
  record,
  rustHelperName,
  rustNamedReference,
  type RustDefinitions,
  type RustProperty,
  type Schema,
} from './RustContractSchema.js';
import { rustNullableText, rustTextProperty } from './RustContractText.js';

const OPTIONAL_MEMBER_ATTRIBUTE = `#[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]`;

/** Delegate a named member to its own type; Rust enums need no check beyond decoding. */
function referenceProperty(name: string, access: string, definitions: RustDefinitions) {
  return { type: name, valid: definitions.enums.has(name) ? [] : [`${access}.valid()`] };
}

/** Bound a list of named items or of a closed string enum. */
function arrayProperty(
  node: Schema,
  access: string,
  definitions: RustDefinitions,
  helperName: string,
): RustProperty {
  const array = contractArraySchema(node);
  const size = [
    ...(array.minimum === 1
      ? [`!${access}.is_empty()`]
      : array.minimum > 1
        ? [`${access}.len() >= ${array.minimum}`]
        : []),
    `${access}.len() <= ${array.maximum}`,
  ];
  const element = rustNamedReference(array.items, definitions.names);
  // Named object items delegate to their own generated bounds; inline shapes stay refused.
  if (element)
    return {
      type: `Vec<${element}>`,
      valid: definitions.enums.has(element)
        ? size
        : [...size, `${access}.iter().all(${element}::valid)`],
    };
  const choices = contractArrayEnum(array.items);
  if (array.maximum === 0) return { type: 'Vec<String>', valid: [`${access}.is_empty()`] };
  if (array.minimum > 0)
    throw new Error('Rust contract enum arrays do not support a minimum length.');
  const condition = `        if !matches!(item.as_str(), ${choices.map((choice) => JSON.stringify(choice)).join(' | ')}) {`;
  const perLine = choices
    .map(
      (choice, index) =>
        `${index ? '                | ' : '            '}${JSON.stringify(choice)}`,
    )
    .join('\n');
  if (perLine.split('\n').some((line) => line.length > 100))
    throw new Error('Rust contract enum needs a bounded compact validator.');
  // rustfmt keeps a wrapped argument's alternatives on one line whenever that line fits.
  const joined = `            ${choices.map((choice) => JSON.stringify(choice)).join(' | ')}`;
  const patterns = joined.length <= 100 ? joined : perLine;
  return {
    type: 'Vec<String>',
    valid: [`${helperName}(&${access})`],
    helper: `fn ${helperName}(value: &[String]) -> bool {
    if value.len() > ${array.maximum} {
        return false;
    }
    for item in value {
${condition.length <= 100 ? condition : `        if !matches!(\n            item.as_str(),\n${patterns}\n        ) {`}
            return false;
        }
    }
    true
}`,
  };
}

/** Compile `T | null` for a named definition or bounded text; other nullable shapes are refused. */
function nullableProperty(
  branches: readonly unknown[],
  access: string,
  definitions: RustDefinitions,
): RustProperty {
  const values = branches.map(record);
  const nulls = values.filter(
    (value) => Object.keys(value).join() === 'const' && value.const === null,
  );
  const present = values.find((value) => !nulls.includes(value));
  if (values.length !== 2 || nulls.length !== 1 || !present)
    throw new Error('Unsupported Rust contract union.');
  const reference = rustNamedReference(present, definitions.names);
  if (reference)
    return {
      type: `Option<${reference}>`,
      requiredOption: true,
      valid: definitions.enums.has(reference)
        ? []
        : [`${access}.as_ref().is_none_or(${reference}::valid)`],
    };
  const text = rustNullableText(present, access);
  if (!text) throw new Error('Unsupported Rust contract union.');
  return text;
}

/** Bound a closed string enum inline, moving long alternatives into a member-local helper. */
function enumProperty(node: Schema, access: string, helperName: string): RustProperty {
  keys(node, ['enum']);
  if (
    !Array.isArray(node.enum) ||
    !node.enum.length ||
    node.enum.some(
      (item: unknown) => typeof item !== 'string' || !/^[a-z][a-z0-9-]*$/.test(item),
    ) ||
    new Set(node.enum).size !== node.enum.length
  )
    throw new Error('Invalid Rust contract string enum.');
  const choices = node.enum.map((item: unknown) => JSON.stringify(item)).join(' | ');
  const direct = `matches!(${access}.as_str(), ${choices})`;
  if (direct.length <= 68) return { type: 'String', valid: [direct] };
  const compact = `    matches!(value, ${choices})`;
  // rustfmt breaks a long alternative chain before every `|`, one token per line.
  const perLine = node.enum
    .map(
      (item: unknown, index) => `${index ? '            | ' : '        '}${JSON.stringify(item)}`,
    )
    .join('\n');
  if (perLine.split('\n').some((line) => line.length > 100))
    throw new Error('Rust contract enum needs a bounded compact validator.');
  const wrapped = `        ${choices}`.length <= 100 ? `        ${choices}` : perLine;
  return {
    type: 'String',
    valid: [`${helperName}(&${access})`],
    helper: `fn ${helperName}(value: &str) -> bool {\n${compact.length <= 100 ? compact : `    matches!(\n        value,\n${wrapped}\n    )`}\n}`,
  };
}

/** Emit only identifiers and literals after validating the closed subset we support. */
export function rustProperty(
  value: unknown,
  field: string,
  definitions: RustDefinitions,
  owner: string,
): RustProperty {
  const node = record(value);
  const access = `self.${field}`;
  const helperName = rustHelperName(owner, field);
  if (node.type === 'array') return arrayProperty(node, access, definitions, helperName);
  if (typeof node.$ref === 'string') {
    keys(node, ['$ref']);
    const reference = rustNamedReference(node, definitions.names);
    if (!reference) throw new Error('Invalid Rust contract reference.');
    return referenceProperty(reference, access, definitions);
  }
  if (typeof node.const === 'string') {
    keys(node, ['const']);
    if (!/^[a-z0-9._-]+$/.test(node.const)) throw new Error('Invalid Rust contract constant.');
    return { type: 'String', valid: [`${access} == ${JSON.stringify(node.const)}`] };
  }
  if (typeof node.const === 'number') {
    keys(node, ['const']);
    if (!Number.isSafeInteger(node.const) || node.const < 0)
      throw new Error('Invalid Rust contract integer constant.');
    return { type: 'u64', valid: [`${access} == ${node.const}`] };
  }
  if (node.const === null) {
    keys(node, ['const']);
    return { type: '()', valid: [] };
  }
  if (Array.isArray(node.oneOf)) {
    keys(node, ['oneOf']);
    return nullableProperty(node.oneOf, access, definitions);
  }
  if (Array.isArray(node.enum)) return enumProperty(node, access, helperName);
  if (node.type === 'string') return rustTextProperty(node, access);
  if (node.type === 'boolean') {
    keys(node, ['type']);
    return { type: 'bool', valid: [] };
  }
  if (node.type === 'number') {
    keys(node, ['type', 'minimum', 'exclusiveMinimum', 'maximum']);
    const hasMinimum = node.minimum !== undefined;
    const hasExclusive = node.exclusiveMinimum !== undefined;
    if (hasMinimum === hasExclusive) throw new Error('Rust contract number needs one lower bound.');
    const lower = literal(hasMinimum ? node.minimum : node.exclusiveMinimum);
    const upper = literal(node.maximum);
    return {
      type: 'f64',
      valid: [
        `(${lower}..=${upper}).contains(&${access})`,
        ...(hasExclusive ? [`${access} > ${lower}`] : []),
      ],
    };
  }
  if (node.type === 'integer') {
    keys(node, ['type', 'minimum', 'maximum']);
    if (
      typeof node.minimum !== 'number' ||
      typeof node.maximum !== 'number' ||
      !Number.isSafeInteger(node.minimum) ||
      !Number.isSafeInteger(node.maximum) ||
      node.maximum < node.minimum
    )
      throw new Error('Rust contract integer needs finite integer bounds.');
    // Signed offsets decode as i64; every nonnegative range keeps the unsigned wire type.
    return {
      type: node.minimum < 0 ? 'i64' : 'u64',
      valid: [`(${node.minimum}..=${node.maximum}).contains(&${access})`],
    };
  }
  throw new Error('Unsupported Rust contract schema property.');
}

/** Emit a present-only bounded list check as a helper, wrapped the way rustfmt lays it out. */
function optionalListHelper(
  name: string,
  element: string,
  minimum: number,
  maximum: number,
): string {
  const parameter = `value: Option<&[${element}]>`;
  const single = `fn ${name}(${parameter}) -> bool {`;
  const signature = single.length <= 100 ? single : `fn ${name}(\n    ${parameter},\n) -> bool {`;
  const size = `${minimum ? `items.len() >= ${minimum} && ` : ''}items.len() <= ${maximum}`;
  const condition = `${size} && items.iter().all(${element}::valid)`;
  const inline = `    value.is_none_or(|items| ${condition})`;
  const body =
    inline.length <= 100 ? inline : `    value.is_none_or(|items| {\n        ${condition}\n    })`;
  return `${signature}\n${body}\n}`;
}

/**
 * Compile an omitted-or-present member: a named definition, a bounded list of a named
 * definition, or a boolean. Validation delegates to the referenced type instead of
 * reinterpreting inline bounds for a wrapped value; other inline shapes stay refused.
 */
export function rustOptionalProperty(
  value: unknown,
  field: string,
  definitions: RustDefinitions,
  owner: string,
): RustProperty & { readonly attribute: string } {
  const node = record(value);
  const reference = rustNamedReference(node, definitions.names);
  if (reference)
    return {
      type: `Option<${reference}>`,
      valid: [`self.${field}.as_ref().is_none_or(${reference}::valid)`],
      attribute: OPTIONAL_MEMBER_ATTRIBUTE,
    };
  if (node.type === 'boolean' && Object.keys(node).join() === 'type')
    return { type: 'Option<bool>', valid: [], attribute: OPTIONAL_MEMBER_ATTRIBUTE };
  const element =
    node.type === 'array'
      ? rustNamedReference(contractArraySchema(node).items, definitions.names)
      : undefined;
  if (element) {
    const array = contractArraySchema(node);
    const helperName = rustHelperName(owner, field);
    return {
      type: `Option<Vec<${element}>>`,
      valid: [`${helperName}(self.${field}.as_deref())`],
      attribute: OPTIONAL_MEMBER_ATTRIBUTE,
      helper: optionalListHelper(helperName, element, array.minimum, array.maximum),
    };
  }
  throw new Error(
    'Rust contract optional members must reference a named definition, list one, or be boolean.',
  );
}
