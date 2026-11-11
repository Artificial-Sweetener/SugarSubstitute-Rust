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
/** Share the closed schema vocabulary and naming rules of every Rust contract emitter. */

export type Schema = Record<string, unknown>;

/** Name the definitions one generated Rust module declares, by the Rust shape each one takes. */
export interface RustDefinitions {
  /** Every declared definition, including string enums. */
  readonly names: ReadonlySet<string>;
  /** Definitions emitted as Rust enums whose decoding already enforces the schema. */
  readonly enums: ReadonlySet<string>;
}

/** Describe one compiled member: its Rust type, its bound checks and any shared helper. */
export interface RustProperty {
  readonly type: string;
  readonly valid: readonly string[];
  /** A required member whose schema admits null; serde must not default it when missing. */
  readonly requiredOption?: boolean;
  readonly helper?: string;
}

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
export function record(value: unknown): Schema {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid Rust contract schema node.');
  return value as Schema;
}

/** Refuse keywords whose meaning a generated decoder would silently drop. */
export function keys(node: Schema, allowed: readonly string[]): void {
  if (Object.keys(node).some((key) => !allowed.includes(key)))
    throw new Error('Unsupported Rust contract schema keyword.');
}

/** Translate a camel-case wire member into a snake-case Rust field, escaping keywords. */
export function rustName(name: string): string {
  if (!/^[a-z][a-zA-Z0-9]*$/.test(name)) throw new Error('Invalid Rust contract field name.');
  const field = name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
  if (['self', 'super', 'crate'].includes(field))
    throw new Error('Unsupported Rust contract field.');
  return RUST_KEYWORDS.has(field) ? `r#${field}` : field;
}

/** Print a finite float bound so Rust infers `f64` for integral values. */
export function literal(value: unknown): string {
  if (typeof value !== 'number' || !Number.isFinite(value))
    throw new Error('Rust contract number needs a finite bound.');
  const result = String(value);
  return Number.isInteger(value) && !result.includes('e') ? `${result}.0` : result;
}

/** Name a member-local helper so variants reusing a field name keep distinct validators. */
export function rustHelperName(owner: string, field: string): string {
  const prefix = owner.replace(
    /[A-Z]/g,
    (letter, index: number) => `${index ? '_' : ''}${letter.toLowerCase()}`,
  );
  return `valid_${prefix}_${field.replace(/^r#/, '')}`;
}

/** Resolve a node that is exactly a reference to a named definition of this module. */
export function rustNamedReference(node: Schema, names: ReadonlySet<string>): string | undefined {
  const reference =
    Object.keys(node).join() === '$ref' && typeof node.$ref === 'string'
      ? /^#\/\$defs\/([A-Za-z]+)$/.exec(node.$ref)?.[1]
      : undefined;
  return reference && names.has(reference) ? reference : undefined;
}
