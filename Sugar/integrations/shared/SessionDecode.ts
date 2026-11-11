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
/** Decode bounded JSON while rejecting duplicate object members before semantic validation. */
import type {
  SessionCheckpoint,
  SessionMessage,
  SessionScope,
} from '../contracts/SessionProtocol.generated.js';
import { utf8Bytes } from './SessionPrimitives.js';
import { isDesktopCheckpoint } from './SessionSnapshot.js';
import { SESSION_MESSAGE_BYTES, sessionMessageError } from './SessionProtocol.js';

/** Check member uniqueness without interpreting nested Comfy graph strings. */
function uniqueMembers(text: string): boolean {
  let offset = 0;
  /** Skip JSON whitespace without accepting non-JSON Unicode whitespace. */
  function space(): void {
    while (/[\x20\t\r\n]/.test(text[offset] ?? '\0')) offset++;
  }
  /** Read a JSON string token, letting JSON.parse validate its escapes. */
  function string(): string {
    const start = offset++;
    while (offset < text.length) {
      const character = text[offset++];
      if (character === '\\') offset++;
      else if (character === '"') return text.slice(start, offset);
    }
    throw new Error('Unterminated JSON string');
  }
  /** Walk one JSON value with bounded nesting and per-object duplicate detection. */
  function value(depth: number): void {
    if (depth > 64) throw new Error('JSON nesting exceeds session bound');
    space();
    const open = text[offset];
    if (open === '"') {
      string();
      return;
    }
    if (open === '{' || open === '[') {
      const close = open === '{' ? '}' : ']';
      const keys = new Set<string>();
      offset++;
      space();
      if (text[offset] === close) {
        offset++;
        return;
      }
      while (offset < text.length) {
        if (open === '{') {
          space();
          if (text[offset] !== '"') throw new Error('Expected JSON member');
          const key: unknown = JSON.parse(string());
          if (typeof key !== 'string' || keys.has(key)) throw new Error('Duplicate JSON member');
          keys.add(key);
          space();
          if (text[offset++] !== ':') throw new Error('Expected JSON colon');
        }
        value(depth + 1);
        space();
        if (text[offset] === close) {
          offset++;
          return;
        }
        if (text[offset++] !== ',') throw new Error('Expected JSON comma');
      }
      throw new Error('Unterminated JSON container');
    }
    const start = offset;
    while (offset < text.length && !/[\x20\t\r\n,\]}]/.test(text[offset] ?? '')) offset++;
    if (offset === start) throw new Error('Missing JSON value');
  }
  try {
    value(0);
    space();
    return offset === text.length;
  } catch {
    return false;
  }
}

/** Parse only bounded, well-formed, duplicate-free wire JSON. */
export function parseSessionJson(text: string, maximum: number): unknown {
  if (text.length > maximum || utf8Bytes(text) > maximum || !uniqueMembers(text)) return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

/** Read an authoritative v1 checkpoint; the storage owner retains rejected original bytes. */
export function decodeDesktopCheckpoint(text: string): SessionCheckpoint | null {
  const value = parseSessionJson(text, 134217728);
  return isDesktopCheckpoint(value) ? value : null;
}

/** Read a message against current independent trust facts, before application admission. */
export function decodeSessionMessage(
  text: string,
  scope: SessionScope,
  origin: string,
): SessionMessage | null {
  const value = parseSessionJson(text, SESSION_MESSAGE_BYTES);
  return sessionMessageError(value, scope, origin) === null ? (value as SessionMessage) : null;
}
