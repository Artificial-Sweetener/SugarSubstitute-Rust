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
/** Generate Rust decoders from the same closed schemas as the web guards. */

import { createHash } from 'node:crypto';
import { contractArrayEnum, contractArraySchema } from './ContractArraySchema.js';
import { contractObjectProperties } from './ContractObjectSchema.js';
import { rustTaggedUnion } from './RustTaggedUnion.js';

type Schema = Record<string, unknown>;
const WELL_FORMED_TEXT = '^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$';
const SURFACE_IDENTITY_TAIL =
  '(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])';
const REQUIRED_OPTION_ATTRIBUTE = '#[serde(deserialize_with = "deserialize_required_option")]';
const OPTIONAL_MEMBER_ATTRIBUTE = `#[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]`;
const SESSION_PATTERN = '^[a-zA-Z0-9-]{16,80}$';
const COLOR_PATTERN = '^#[a-fA-F0-9]{6}$';
const BASE64_PATTERN =
  '^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?(?![\\s\\S])';
const RUST_KEYWORDS = new Set([
  'as',
  'async',
  'await',
  'break',
  'const',
  'continue',
  'dyn',
  'else',
  'enum',
  'extern',
  'false',
  'fn',
  'for',
  'gen',
  'if',
  'impl',
  'in',
  'let',
  'loop',
  'match',
  'mod',
  'move',
  'mut',
  'pub',
  'ref',
  'return',
  'static',
  'struct',
  'trait',
  'true',
  'type',
  'unsafe',
  'use',
  'where',
  'while',
  'abstract',
  'become',
  'box',
  'do',
  'final',
  'macro',
  'override',
  'priv',
  'try',
  'typeof',
  'unsized',
  'virtual',
  'yield',
]);

/** Fail closed when the schema vocabulary changes instead of emitting a weaker decoder. */
function record(value: unknown): Schema {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid Rust contract schema node.');
  return value as Schema;
}

function keys(node: Schema, allowed: readonly string[]): void {
  if (Object.keys(node).some((key) => !allowed.includes(key)))
    throw new Error('Unsupported Rust contract schema keyword.');
}

function rustName(name: string): string {
  if (!/^[a-z][a-zA-Z0-9]*$/.test(name)) throw new Error('Invalid Rust contract field name.');
  const field = name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
  if (['self', 'super', 'crate'].includes(field))
    throw new Error('Unsupported Rust contract field.');
  return RUST_KEYWORDS.has(field) ? `r#${field}` : field;
}

function literal(value: unknown): string {
  if (typeof value !== 'number' || !Number.isFinite(value))
    throw new Error('Rust contract number needs a finite bound.');
  const result = String(value);
  return Number.isInteger(value) && !result.includes('e') ? `${result}.0` : result;
}

/** Emit only identifiers and literals after validating the closed subset we support. */
function property(
  value: unknown,
  field: string,
  names: ReadonlySet<string>,
  owner: string,
): { type: string; valid: readonly string[]; requiredOption?: boolean; helper?: string } {
  const node = record(value);
  const access = `self.${field}`;
  if (node.type === 'array') {
    const array = contractArraySchema(node);
    const choices = contractArrayEnum(array.items);
    if (array.maximum === 0) return { type: 'Vec<String>', valid: [`${access}.is_empty()`] };
    const condition = `        if !matches!(item.as_str(), ${choices.map((choice) => JSON.stringify(choice)).join(' | ')}) {`;
    const patterns = choices
      .map(
        (choice, index) =>
          `${index ? '                | ' : '            '}${JSON.stringify(choice)}`,
      )
      .join('\n');
    if (patterns.split('\n').some((line) => line.length > 100))
      throw new Error('Rust contract enum needs a bounded compact validator.');
    const helperName = `valid_${owner.replace(/[A-Z]/g, (letter, index) => `${index ? '_' : ''}${letter.toLowerCase()}`)}_${field.replace(/^r#/, '')}`;
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
  if (typeof node.$ref === 'string') {
    keys(node, ['$ref']);
    const reference = /^#\/\$defs\/([A-Za-z]+)$/.exec(node.$ref)?.[1];
    if (!reference || !names.has(reference)) throw new Error('Invalid Rust contract reference.');
    return { type: reference, valid: [`${access}.valid()`] };
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
    if (node.oneOf.length !== 2) throw new Error('Unsupported Rust contract union.');
    const text = record(node.oneOf[0]);
    const none = record(node.oneOf[1]);
    keys(text, ['type', 'maxLength', 'pattern']);
    keys(none, ['const']);
    if (
      text.type !== 'string' ||
      text.maxLength !== 7 ||
      text.pattern !== COLOR_PATTERN ||
      none.const !== null
    )
      throw new Error('Unsupported Rust contract union.');
    return {
      type: 'Option<String>',
      requiredOption: true,
      valid: [`valid_optional_color(${access}.as_deref())`],
      helper: `pub(super) fn valid_optional_color(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    })
}`,
    };
  }
  if (Array.isArray(node.enum)) {
    keys(node, ['enum']);
    if (
      !node.enum.length ||
      node.enum.some((item: unknown) => typeof item !== 'string' || !/^[a-z-]+$/.test(item)) ||
      new Set(node.enum).size !== node.enum.length
    )
      throw new Error('Invalid Rust contract string enum.');
    const choices = node.enum.map((item: unknown) => JSON.stringify(item)).join(' | ');
    const direct = `matches!(${access}.as_str(), ${choices})`;
    if (direct.length > 68) {
      const helperName = `valid_${owner.replace(/[A-Z]/g, (letter, index) => `${index ? '_' : ''}${letter.toLowerCase()}`)}_${field.replace(/^r#/, '')}`;
      if (`    matches!(value, ${choices})`.length > 100)
        throw new Error('Rust contract enum needs a bounded compact validator.');
      return {
        type: 'String',
        valid: [`${helperName}(&${access})`],
        helper: `fn ${helperName}(value: &str) -> bool {\n    matches!(value, ${choices})\n}`,
      };
    }
    return {
      type: 'String',
      valid: [direct],
    };
  }
  if (node.type === 'string') {
    keys(node, ['type', 'maxLength', 'pattern', 'x-length-unit']);
    if (
      node.pattern === BASE64_PATTERN &&
      typeof node.maxLength === 'number' &&
      Number.isSafeInteger(node.maxLength) &&
      node.maxLength > 0 &&
      node.maxLength <= 10924 &&
      node['x-length-unit'] === undefined
    )
      return {
        type: 'String',
        valid: [`valid_bounded_base64(&${access}, ${node.maxLength})`],
        helper: `fn valid_bounded_base64(value: &str, maximum: usize) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() > maximum || !bytes.len().is_multiple_of(4) {
        return false;
    }
    let padding = if value.ends_with("==") { 2 } else { usize::from(value.ends_with('=')) };
    bytes[..bytes.len() - padding]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}`,
      };
    const identityPrefix = ['canvas:', 'view:', 'window:', 'attachment:', 'result:'].find(
      (prefix) => node.pattern === `^${prefix}${SURFACE_IDENTITY_TAIL}`,
    );
    if (
      identityPrefix &&
      typeof node.maxLength === 'number' &&
      Number.isSafeInteger(node.maxLength) &&
      node.maxLength >= identityPrefix.length + 1 &&
      node.maxLength <= 4096 &&
      node['x-length-unit'] === 'unicode-codepoints'
    )
      return {
        type: 'String',
        valid: [
          `valid_surface_identity(&${access}, ${JSON.stringify(identityPrefix)}, ${node.maxLength})`,
        ],
        helper: `fn valid_surface_identity(value: &str, prefix: &str, maximum: usize) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|tail| !tail.is_empty() && !tail.chars().any(char::is_control))
        && value.chars().count() <= maximum
}`,
      };
    if (
      node.maxLength === 80 &&
      node.pattern === SESSION_PATTERN &&
      node['x-length-unit'] === undefined
    )
      return {
        type: 'String',
        valid: [`valid_panel_session(&${access})`],
        helper: `pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}`,
      };
    if (
      typeof node.maxLength === 'number' &&
      Number.isSafeInteger(node.maxLength) &&
      node.maxLength >= 0 &&
      node.maxLength <= 4096 &&
      node.pattern === undefined &&
      node['x-length-unit'] === undefined
    )
      return { type: 'String', valid: [`${access}.encode_utf16().count() <= ${node.maxLength}`] };
    if (
      typeof node.maxLength !== 'number' ||
      !Number.isSafeInteger(node.maxLength) ||
      node.maxLength < 0 ||
      node.maxLength > 4096 ||
      node['x-length-unit'] !== 'unicode-codepoints' ||
      node.pattern !== WELL_FORMED_TEXT
    )
      throw new Error('Rust contract strings require bounded, well-formed text.');
    return { type: 'String', valid: [`${access}.chars().count() <= ${node.maxLength}`] };
  }
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
      node.minimum < 0 ||
      node.maximum < node.minimum
    )
      throw new Error('Rust contract integer needs nonnegative finite bounds.');
    return { type: 'u64', valid: [`(${node.minimum}..=${node.maximum}).contains(&${access})`] };
  }
  throw new Error('Unsupported Rust contract schema property.');
}

/**
 * Compile an omitted-or-present member. Only named definitions may be optional, so validation
 * delegates to the referenced type instead of reinterpreting inline bounds for a wrapped value.
 */
function optionalProperty(
  value: unknown,
  field: string,
  names: ReadonlySet<string>,
): { type: string; valid: readonly string[]; attribute: string } {
  const node = record(value);
  const reference =
    Object.keys(node).join() === '$ref' && typeof node.$ref === 'string'
      ? /^#\/\$defs\/([A-Za-z]+)$/.exec(node.$ref)?.[1]
      : undefined;
  if (!reference || !names.has(reference))
    throw new Error('Rust contract optional members must reference a named definition.');
  return {
    type: `Option<${reference}>`,
    valid: [`self.${field}.as_ref().is_none_or(${reference}::valid)`],
    attribute: OPTIONAL_MEMBER_ATTRIBUTE,
  };
}

/** Compile only closed objects; admission and navigation authority remain host-owned. */
export function rustContractBindings(
  definitions: Schema,
  schemaText: string,
  options: { readonly schema: string; readonly generator: string; readonly prefix: string },
): string {
  if (
    !/^[a-z-]+\.schema\.json$/.test(options.schema) ||
    !/^scripts\/integrations\/generate-[a-z-]+-contract\.ts$/.test(options.generator) ||
    !/^[A-Za-z]+$/.test(options.prefix)
  )
    throw new Error('Invalid Rust contract source.');
  const names = new Set(Object.keys(definitions));
  const helpers = new Set<string>();
  let requiredOption = false;
  let optionalMember = false;
  const sourceHash = createHash('sha256').update(schemaText.replaceAll('\r\n', '\n')).digest('hex');
  const header = `// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from ${options.schema} by ${options.generator}.
// Schema SHA-256 (LF): ${sourceHash}
`;
  const values = Object.entries(definitions).map(([name, value]) => {
    if (!name.startsWith(options.prefix) || !/^[A-Za-z]+$/.test(name))
      throw new Error('Invalid Rust contract type name.');
    const node = record(value);
    if (Array.isArray(node.oneOf))
      return rustTaggedUnion(name, node, (key, item, variant) => {
        const field = rustName(key);
        const result = property(item, field, names, `${name}${variant}`);
        if (result.helper) helpers.add(result.helper);
        return { field, ...result };
      });
    keys(node, ['type', 'properties', 'required', 'additionalProperties']);
    const fields = contractObjectProperties(node).map(({ key, value: item, optional }) => {
      const field = rustName(key);
      if (optional) {
        optionalMember = true;
        return { field, ...optionalProperty(item, field, names) };
      }
      const result = property(item, field, names, name);
      requiredOption ||= result.requiredOption === true;
      if (result.helper) helpers.add(result.helper);
      return {
        field,
        ...result,
        attribute: result.requiredOption ? REQUIRED_OPTION_ATTRIBUTE : null,
      };
    });
    const checks = fields.flatMap(({ valid }) => valid);
    const inline = checks.join(' && ');
    const validation = !checks.length
      ? 'true'
      : inline.length <= 92
        ? inline
        : checks.join('\n            && ');
    return `/// Schema-defined untrusted ${name} data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ${name} {
${fields.map(({ field, type, attribute }) => `${attribute ? `    ${attribute}\n` : ''}    pub(super) ${field}: ${type},`).join('\n')}
}
impl ${name} {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        ${validation}
    }
}`;
  });
  const helper = requiredOption
    ? `\n/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}\n`
    : '';
  const present = optionalMember
    ? `\n/// Decode an optional member only when present, rejecting an explicit null.
fn deserialize_present_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}\n`
    : '';
  return `${header}\n${values.join('\n\n')}\n${helpers.size ? `\n${[...helpers].join('\n\n')}\n` : ''}${helper}${present}`;
}
