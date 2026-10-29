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
  readonly requiredOption?: boolean;
}

/** Reject ambiguous tags and unsupported branch shapes before emitting any Rust. */
function branches(value: Schema): readonly { tag: string; properties: Schema }[] {
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
    const discriminator = fields.type;
    if (
      !discriminator ||
      typeof discriminator !== 'object' ||
      Array.isArray(discriminator) ||
      Object.keys(discriminator).join() !== 'const'
    )
      throw new Error('Rust contract tagged branches need an exact type constant.');
    const tag = (discriminator as Schema).const;
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
      properties: Object.fromEntries(Object.entries(fields).filter(([k]) => k !== 'type')),
    };
  });
}

/** Preserve closed branches, including empty struct variants which reject extra members. */
export function rustTaggedUnion(
  name: string,
  schema: Schema,
  compile: (key: string, value: unknown, variant: string) => Field,
): string {
  const variants = branches(schema).map(({ tag, properties }) => {
    const variant = tag.replace(/(^|-)([a-z])/g, (_, _separator: string, letter: string) =>
      letter.toUpperCase(),
    );
    const fields = Object.entries(properties).map(([key, value]) => compile(key, value, variant));
    if (fields.some(({ requiredOption }) => requiredOption))
      throw new Error('Rust contract tagged branches do not support nullable members.');
    const checks = fields.flatMap(({ field, type, valid }) =>
      valid.map((check) =>
        check
          .replaceAll(`&self.${field}`, field)
          .replaceAll(
            `self.${field} ==`,
            type === 'String' ? `${field}.as_str() ==` : `(*${field}) ==`,
          )
          .replaceAll(
            `self.${field}`,
            ['f64', 'u64', 'bool', '()'].includes(type) ? `(*${field})` : field,
          ),
      ),
    );
    const pattern = fields
      .map(({ field, valid }) => (valid.length ? field : `${field}: _`))
      .join(', ');
    const validation = checks.length ? checks.join(' && ') : 'true';
    const shape = pattern ? `{ ${pattern} }` : '{}';
    const arm = `            Self::${variant} ${shape} => ${validation},`;
    const members = fields.map(({ field, type }) => `${field}: ${type}`).join(', ');
    const declaration = `    ${variant} {${members ? ` ${members} ` : ''}},`;
    return {
      declaration: `    #[serde(rename = ${JSON.stringify(tag)})]\n${declaration.length <= 100 ? declaration : `    ${variant} {\n${fields.map(({ field, type }) => `        ${field}: ${type},`).join('\n')}\n    },`}`,
      arm:
        arm.length <= 100
          ? arm
          : `            Self::${variant} ${shape} => {\n                ${checks.join('\n                    && ')}\n            }`,
    };
  });
  return `/// Schema-defined untrusted ${name} data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
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
