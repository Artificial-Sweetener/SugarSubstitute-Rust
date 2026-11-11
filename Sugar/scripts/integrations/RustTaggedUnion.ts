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
/** Compile bounded, disjoint closed object unions through Rust's explicit wire tag. */

type Schema = Record<string, unknown>;
interface Field {
  readonly field: string;
  readonly type: string;
  readonly valid: readonly string[];
  /** A serde field attribute, such as the required-nullable decoder, placed above the member. */
  readonly attribute?: string | null;
}
const SCALARS = ['f64', 'u64', 'i64', 'bool', '()'];

/**
 * Read the wire tag: the first member of the first branch when every branch fixes it to a string.
 *
 * Commands and messages use `type`, results use `kind` and choices such as the memory preference
 * use `mode`. Unions without a common leading constant keep `type`, which branch validation then
 * requires of every branch.
 */
function discriminator(value: Schema): string {
  const branches = Array.isArray(value.oneOf) ? (value.oneOf as unknown[]) : [];
  const leading = (branch: unknown): string | undefined => {
    const properties =
      branch && typeof branch === 'object' && !Array.isArray(branch)
        ? (branch as Schema).properties
        : undefined;
    return properties && typeof properties === 'object' ? Object.keys(properties)[0] : undefined;
  };
  const key = leading(branches[0]);
  const fixed = (branch: unknown): boolean => {
    const properties = (branch as Schema).properties as Schema;
    const member = key === undefined ? undefined : properties[key];
    return (
      !!member &&
      typeof member === 'object' &&
      !Array.isArray(member) &&
      typeof (member as Schema).const === 'string'
    );
  };
  return key &&
    /^[a-z]+$/.test(key) &&
    branches.every((branch) => leading(branch) === key && fixed(branch))
    ? key
    : 'type';
}

/** Reject ambiguous tags and unsupported branch shapes before emitting any Rust. */
function branches(value: Schema, tagKey: string): readonly { tag: string; properties: Schema }[] {
  if (
    Object.keys(value).length !== 1 ||
    !Array.isArray(value.oneOf) ||
    value.oneOf.length < 2 ||
    value.oneOf.length > 16
  )
    throw new Error('Rust contract tagged unions need 2..16 closed branches.');
  const tags = new Set<string>();
  return value.oneOf.map((value: unknown) => {
    if (!value || typeof value !== 'object' || Array.isArray(value))
      throw new Error('Invalid Rust contract tagged branch.');
    const branch = value as Schema;
    const properties = branch.properties;
    if (
      Object.keys(branch).some(
        (key) => !['type', 'properties', 'required', 'additionalProperties'].includes(key),
      ) ||
      branch.type !== 'object' ||
      branch.additionalProperties !== false ||
      !properties ||
      typeof properties !== 'object' ||
      Array.isArray(properties) ||
      JSON.stringify(branch.required) !== JSON.stringify(Object.keys(properties))
    )
      throw new Error('Rust contract tagged branches must be closed and fully required.');
    const fields = properties as Schema;
    const tagSchema = fields[tagKey];
    if (
      !tagSchema ||
      typeof tagSchema !== 'object' ||
      Array.isArray(tagSchema) ||
      Object.keys(tagSchema).join() !== 'const'
    )
      throw new Error('Rust contract tagged branches need an exact type constant.');
    const tag = (tagSchema as Schema).const;
    if (
      typeof tag !== 'string' ||
      tag.length > 64 ||
      !/^[a-z]+(?:-[a-z]+)*$/.test(tag) ||
      tag === 'self' ||
      tags.has(tag)
    )
      throw new Error('Rust contract tagged branches need unique safe tags.');
    tags.add(tag);
    return {
      tag,
      properties: Object.fromEntries(Object.entries(fields).filter(([k]) => k !== tagKey)),
    };
  });
}

/** Rebind a struct-style check to the borrowed member a match pattern binds. */
function bindCheck(check: string, { field, type }: Field): string {
  const member = `self\\.${field.replace(/[^A-Za-z0-9_]/g, (character) => `\\${character}`)}\\b`;
  return check
    .replace(new RegExp(`&${member}`, 'g'), field)
    .replace(
      new RegExp(`${member} ==`, 'g'),
      type === 'String' ? `${field}.as_str() ==` : `(*${field}) ==`,
    )
    .replace(new RegExp(member, 'g'), SCALARS.includes(type) ? `(*${field})` : field);
}

/** Lay out one variant as rustfmt does, keeping attributed members on their own lines. */
function declaration(variant: string, fields: readonly Field[]): string {
  const members = fields.map(({ field, type }) => `${field}: ${type}`).join(', ');
  const single = `    ${variant} {${members ? ` ${members} ` : ''}},`;
  if (single.length <= 100 && fields.every(({ attribute }) => !attribute)) return single;
  const lines = fields.map(
    ({ field, type, attribute }) =>
      `${attribute ? `        ${attribute}\n` : ''}        ${field}: ${type},`,
  );
  return `    ${variant} {\n${lines.join('\n')}\n    },`;
}

/** Lay out one validation arm as rustfmt does for short and wrapped patterns and bodies. */
function arm(variant: string, fields: readonly Field[], checks: readonly string[]): string {
  const bindings = fields.map(({ field, valid }) => (valid.length ? field : `${field}: _`));
  const validation = checks.length ? checks.join(' && ') : 'true';
  const shape = bindings.length ? `{ ${bindings.join(', ')} }` : '{}';
  const single = `            Self::${variant} ${shape} => ${validation},`;
  if (single.length <= 100) return single;
  const wrappedBody = (indent: string) =>
    `${indent}    ${validation.length + indent.length + 4 <= 100 ? validation : checks.join(`\n${indent}        && `)}`;
  const head = `            Self::${variant} ${shape} => {`;
  if (head.length <= 100) return `${head}\n${wrappedBody('            ')}\n            }`;
  const pattern = `            Self::${variant} {\n${bindings.map((binding) => `                ${binding},`).join('\n')}\n            }`;
  const tail = `            } => ${validation},`;
  if (tail.length <= 100) return `${pattern} => ${validation},`;
  return `${pattern} => {\n${wrappedBody('            ')}\n            }`;
}

/** Preserve closed branches, including empty struct variants which reject extra members. */
export function rustTaggedUnion(
  name: string,
  schema: Schema,
  compile: (key: string, value: unknown, variant: string) => Field,
): string {
  const tagKey = discriminator(schema);
  const variants = branches(schema, tagKey).map(({ tag, properties }) => {
    const variant = tag.replace(/(^|-)([a-z])/g, (_, _separator: string, letter: string) =>
      letter.toUpperCase(),
    );
    const fields = Object.entries(properties).map(([key, value]) => compile(key, value, variant));
    const checks = fields.flatMap((field) => field.valid.map((check) => bindCheck(check, field)));
    return {
      declaration: `    #[serde(rename = ${JSON.stringify(tag)})]\n${declaration(variant, fields)}`,
      arm: arm(variant, fields, checks),
    };
  });
  return `/// Schema-defined untrusted ${name} data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "${tagKey}", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum ${name} {
${variants.map(({ declaration }) => declaration).join('\n')}
}
impl ${name} {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
${variants.map(({ arm }) => arm).join('\n')}
        }
    }
}`;
}
