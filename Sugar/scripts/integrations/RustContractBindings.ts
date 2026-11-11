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
import { contractObjectProperties } from './ContractObjectSchema.js';
import { rustOptionalProperty, rustProperty } from './RustContractProperty.js';
import {
  keys,
  record,
  rustName,
  type RustDefinitions,
  type RustProperty,
  type Schema,
} from './RustContractSchema.js';
import { rustStringEnum, rustUntaggedUnion } from './RustNamedUnion.js';
import { rustTaggedUnion } from './RustTaggedUnion.js';

const REQUIRED_OPTION_ATTRIBUTE = '#[serde(deserialize_with = "deserialize_required_option")]';

/** Collect the helpers and serde decoders the emitted declarations depend on. */
interface Emission {
  readonly helpers: Set<string>;
  requiredOption: boolean;
  optionalMember: boolean;
}

/** Retain a member's helper once and report whether it needs the required-nullable decoder. */
function retain(result: RustProperty, emission: Emission): string | null {
  if (result.helper) emission.helpers.add(result.helper);
  if (!result.requiredOption) return null;
  emission.requiredOption = true;
  return REQUIRED_OPTION_ATTRIBUTE;
}

/** Compile one closed object into a struct and a check of every schema bound. */
function rustStruct(
  name: string,
  node: Schema,
  definitions: RustDefinitions,
  emission: Emission,
): string {
  keys(node, ['type', 'properties', 'required', 'additionalProperties']);
  const fields = contractObjectProperties(node).map(({ key, value: item, optional }) => {
    const field = rustName(key);
    if (optional) {
      emission.optionalMember = true;
      const result = rustOptionalProperty(item, field, definitions, name);
      retain(result, emission);
      return { field, ...result };
    }
    const result = rustProperty(item, field, definitions, name);
    return { field, ...result, attribute: retain(result, emission) };
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
}

/** Compile closed objects, unions and string enums; admission and authority remain host-owned. */
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
  const declared: RustDefinitions = {
    names: new Set(Object.keys(definitions)),
    enums: new Set(
      Object.entries(definitions)
        .filter(([, value]) => Array.isArray(record(value).enum))
        .map(([name]) => name),
    ),
  };
  const emission: Emission = { helpers: new Set(), requiredOption: false, optionalMember: false };
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
    if (declared.enums.has(name)) return rustStringEnum(name, node);
    if (!Array.isArray(node.oneOf)) return rustStruct(name, node, declared, emission);
    if (node.oneOf.every((branch: unknown) => Object.keys(record(branch)).join() === '$ref'))
      return rustUntaggedUnion(name, node, definitions);
    return rustTaggedUnion(name, node, (key, item, variant) => {
      const field = rustName(key);
      const result = rustProperty(item, field, declared, `${name}${variant}`);
      return { field, ...result, attribute: retain(result, emission) };
    });
  });
  const required = emission.requiredOption
    ? `\n/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}\n`
    : '';
  const present = emission.optionalMember
    ? `\n/// Decode an optional member only when present, rejecting an explicit null.
fn deserialize_present_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}\n`
    : '';
  const helpers = emission.helpers.size ? `\n${[...emission.helpers].join('\n\n')}\n` : '';
  return `${header}\n${values.join('\n\n')}\n${helpers}${required}${present}`;
}
