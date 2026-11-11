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
/** Compile the bounded text forms the contracts use into equivalent Rust checks. */
import { keys, type RustProperty, type Schema } from './RustContractSchema.js';

const WELL_FORMED_TEXT = '^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$';
const SURFACE_IDENTITY_TAIL =
  '(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])';
const SESSION_PATTERN = '^[a-zA-Z0-9-]{16,80}$';
const COLOR_PATTERN = '^#[a-fA-F0-9]{6}$';
const BASE64_PATTERN =
  '^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?(?![\\s\\S])';
/** Admit unpatterned UTF-16 strings up to the base64 form of the largest canvas raster. */
const MAX_PLAIN_TEXT_UNITS = 89478488;

/** Recognize a closed `#rrggbb` color, the only patterned text admitted without a length unit. */
function isColor(node: Schema): boolean {
  return (
    node.type === 'string' &&
    node.maxLength === 7 &&
    node.pattern === COLOR_PATTERN &&
    Object.keys(node).length === 3
  );
}

/** Recognize a stable lowercase token such as a cache consumer id: `a-z`, `0-9`, `.` and `-`. */
function lowercaseTokenBound(node: Schema): number | undefined {
  const bound = node.maxLength;
  return node.type === 'string' &&
    typeof bound === 'number' &&
    Number.isSafeInteger(bound) &&
    bound >= 1 &&
    bound <= 256 &&
    node.pattern === `^[a-z0-9.-]{1,${bound}}$` &&
    node['x-length-unit'] === undefined
    ? bound
    : undefined;
}

/** Recognize UTF-16 bounded text without a pattern, the form TypeScript checks by `length`. */
function plainBound(node: Schema): number | undefined {
  return node.type === 'string' &&
    typeof node.maxLength === 'number' &&
    Number.isSafeInteger(node.maxLength) &&
    node.maxLength >= 0 &&
    node.maxLength <= MAX_PLAIN_TEXT_UNITS &&
    node.pattern === undefined &&
    node['x-length-unit'] === undefined
    ? node.maxLength
    : undefined;
}

/** Compile one required text member; unknown patterns and unbounded text stay refused. */
export function rustTextProperty(node: Schema, access: string): RustProperty {
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
  const token = lowercaseTokenBound(node);
  if (token !== undefined)
    return {
      type: 'String',
      valid: [`valid_lowercase_token(&${access}, ${token})`],
      helper: `fn valid_lowercase_token(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}`,
    };
  if (isColor(node))
    return {
      type: 'String',
      valid: [`valid_color(&${access})`],
      helper: `fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}`,
    };
  const plain = plainBound(node);
  if (plain !== undefined)
    return { type: 'String', valid: [`${access}.encode_utf16().count() <= ${plain}`] };
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

/** Compile a required-but-nullable color or UTF-16 bounded text member, or refuse it. */
export function rustNullableText(node: Schema, access: string): RustProperty | undefined {
  if (isColor(node))
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
  const plain = plainBound(node);
  if (plain === undefined) return undefined;
  return {
    type: 'Option<String>',
    requiredOption: true,
    valid: [`valid_optional_text(${access}.as_deref(), ${plain})`],
    helper: `fn valid_optional_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| value.encode_utf16().count() <= maximum)
}`,
  };
}
