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
/** Compile untagged unions of named closed objects and named string enums into Rust enums. */
import { contractObjectProperties } from './ContractObjectSchema.js';
import { record, rustNamedReference, type Schema } from './RustContractSchema.js';

/** Split a definition name into its capitalized words. */
function words(name: string): readonly string[] {
  return name.match(/[A-Z][a-z0-9]*/g) ?? [];
}

/** Name variants by the words that distinguish each definition from its siblings. */
function variantNames(references: readonly string[]): readonly string[] {
  const split = references.map(words);
  const shortest = Math.min(...split.map((parts) => parts.length));
  let prefix = 0;
  while (prefix < shortest && split.every((parts) => parts[prefix] === split[0]?.[prefix]))
    prefix++;
  let suffix = 0;
  while (
    suffix < shortest - prefix &&
    split.every((parts) => parts.at(-1 - suffix) === split[0]?.at(-1 - suffix))
  )
    suffix++;
  const names = split.map((parts) => parts.slice(prefix, parts.length - suffix).join(''));
  if (names.some((variant) => !variant) || new Set(names).size !== names.length)
    throw new Error('Rust contract untagged branches need distinguishing definition names.');
  return names;
}

/**
 * Compile a union whose branches are named, closed, fully required objects.
 *
 * Every wire value carries exactly the members of one branch, so distinct member sets make the
 * branches mutually exclusive and serde's first-match untagged decoding is unambiguous.
 */
export function rustUntaggedUnion(
  name: string,
  schema: Schema,
  definitions: Readonly<Record<string, unknown>>,
): string {
  const branches = Array.isArray(schema.oneOf) ? schema.oneOf.map(record) : [];
  const names = new Set(Object.keys(definitions));
  const references = branches.map((branch) => rustNamedReference(branch, names));
  if (
    Object.keys(schema).length !== 1 ||
    branches.length < 2 ||
    branches.length > 16 ||
    references.some((reference) => !reference)
  )
    throw new Error('Rust contract untagged unions need 2..16 named branches.');
  const named = references.filter((reference): reference is string => !!reference);
  const shapes = named.map((reference) => {
    const node = record(definitions[reference]);
    const properties = Array.isArray(node.oneOf) ? [] : contractObjectProperties(node);
    if (!properties.length || properties.some(({ optional }) => optional))
      throw new Error('Rust contract untagged branches must be closed and fully required.');
    return properties
      .map(({ key }) => key)
      .sort()
      .join();
  });
  if (new Set(shapes).size !== shapes.length)
    throw new Error('Rust contract untagged branches must have distinct members.');
  const variants = variantNames(named);
  return `/// Schema-defined untrusted ${name} data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum ${name} {
${named.map((reference, index) => `    ${variants[index] ?? ''}(${reference}),`).join('\n')}
}
impl ${name} {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
${variants.map((variant) => `            Self::${variant}(value) => value.valid(),`).join('\n')}
        }
    }
}`;
}

/** Compile a named closed string enum; decoding rejects every other token. */
export function rustStringEnum(name: string, schema: Schema): string {
  const values = schema.enum;
  if (
    Object.keys(schema).join() !== 'enum' ||
    !Array.isArray(values) ||
    values.length < 1 ||
    values.length > 64 ||
    values.some(
      (value: unknown) => typeof value !== 'string' || !/^[a-z]+(?:-[a-z]+)*$/.test(value),
    ) ||
    new Set(values).size !== values.length
  )
    throw new Error('Rust contract string enums need unique kebab-case tokens.');
  const variants = values.map((value: unknown) =>
    String(value).replace(/(^|-)([a-z])/g, (_, _separator: string, letter: string) =>
      letter.toUpperCase(),
    ),
  );
  return `/// Schema-defined untrusted ${name} token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ${name} {
${variants.map((variant) => `    ${variant},`).join('\n')}
}`;
}
