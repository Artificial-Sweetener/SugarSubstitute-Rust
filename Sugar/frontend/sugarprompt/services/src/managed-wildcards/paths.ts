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
/** Relative wildcard file naming mirrors retained donor POSIX spellings; host adapters enforce real containment. */
import { strip } from '../../../core/src/prompt/text.js';
import { unicodeLowerCase } from '../../../core/src/index.js';
import type { ManagedWildcardFile } from './contracts.js';

/** Normalize relative slash paths without resolving an operating-system path or accepting traversal/drive qualifiers. */
function relative(value: unknown): string | null {
  if (typeof value !== 'string' || value.length > 1024 || value.includes('\0')) return null;
  const normalized = strip(value).replaceAll('\\', '/');
  const parts = normalized.split('/').filter((part) => part && part !== '.');
  if (
    !normalized ||
    normalized.startsWith('/') ||
    !parts.length ||
    parts.some((part) => part === '..' || part.includes(':'))
  )
    return null;
  return parts.join('/');
}
/** Validate a managed file identity and derive its library metadata once without parsing candidate content. */
export function managedWildcardFile(value: unknown): ManagedWildcardFile | null {
  const path = relative(value);
  if (path === null) return null;
  const name = path.slice(path.lastIndexOf('/') + 1),
    dot = name.lastIndexOf('.');
  if (dot < 1) return null;
  const suffix = unicodeLowerCase(name.slice(dot));
  if (suffix !== '.txt' && suffix !== '.csv') return null;
  return Object.freeze({
    path,
    identifier: path.slice(0, path.length - name.length + dot),
    kind: suffix === '.txt' ? 'text' : 'csv',
  });
}
/** Separate TXT/CSV creation accepts an identifier or existing supported suffix and preserves exact default contents. */
export function managedWildcardCreationPath(
  value: unknown,
  kind: ManagedWildcardFile['kind'],
): string | null {
  let identifier = relative(value);
  if (identifier === null) return null;
  const existing = managedWildcardFile(identifier);
  if (existing) identifier = existing.identifier;
  const file = managedWildcardFile(identifier + (kind === 'text' ? '.txt' : '.csv'));
  return file?.path ?? null;
}
