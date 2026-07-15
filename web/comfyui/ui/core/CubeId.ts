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
/**
 * Own the SugarCubes core UI service layer in `web/comfyui/ui/core/CubeId.js`.
 */

const OWNER_RE = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/;
const REPO_RE = /^[A-Za-z0-9._-]+$/;
const LOCAL_NAMESPACE_RE = /^[A-Za-z0-9](?:[A-Za-z0-9._-]{0,62})$/;
const WINDOWS_UNSAFE_FILENAME_RE = /[<>:"|?*]/;
const RESERVED_SOURCE_NAMES = new Set(['local', 'flavors']);
const TITLE_SMALL_WORDS = new Set([
  'a',
  'an',
  'and',
  'as',
  'at',
  'but',
  'by',
  'for',
  'from',
  'in',
  'into',
  'nor',
  'of',
  'on',
  'or',
  'per',
  'the',
  'to',
  'vs',
  'via',
  'with',
]);
const TITLE_TECH_TERMS = new Map([
  ['ai', 'AI'],
  ['clip', 'CLIP'],
  ['ipadapter', 'IPAdapter'],
  ['lora', 'LoRA'],
  ['sd', 'SD'],
  ['sdxl', 'SDXL'],
  ['ui', 'UI'],
  ['vae', 'VAE'],
  ['xl', 'XL'],
]);
const ASCII_LETTER_RE = /[A-Za-z]/;
const ASCII_WORD_RE = /[A-Za-z]+/;
const VERSION_TOKEN_RE = /^v[0-9]+(?:[A-Za-z0-9.-]*)?$/i;

export type CubeSourceKind = 'github' | 'local';

export interface CanonicalCubeId {
  sourceKind: CubeSourceKind;
  owner: string;
  repo: string;
  namespace: string;
  path: string;
  cubeId: string;
  sourceRoot: string;
  repoRef: string;
}

interface TitleTokenPosition {
  index: number;
  isLast: boolean;
}

function hasAsciiControlCharacter(value: string): boolean {
  for (const char of value) {
    const code = char.charCodeAt(0);
    if (code <= 31) {
      return true;
    }
  }
  return false;
}

function validateCubePathSegment(value: unknown): string {
  if (typeof value !== 'string' || !value || value === '.' || value === '..') {
    throw new Error('Cube id path segment is required.');
  }
  if (value.includes('/') || value.includes('\\')) {
    throw new Error('Cube filename must not contain path separators.');
  }
  const nameStem = value.toLowerCase().endsWith('.cube') ? value.slice(0, -5) : value;
  if (value.toLowerCase().endsWith('.cube') && !nameStem) {
    throw new Error('Cube filename is required.');
  }
  if (/[ .]$/.test(value) || /[ .]$/.test(nameStem)) {
    throw new Error('Cube filename must not end with a space or dot.');
  }
  if (WINDOWS_UNSAFE_FILENAME_RE.test(value) || hasAsciiControlCharacter(value)) {
    throw new Error('Cube filename contains invalid characters.');
  }
  return value;
}

/**
 * Parse canonical source-qualified cube id.
 */
export function parseCanonicalCubeId(cubeId: unknown): CanonicalCubeId {
  if (typeof cubeId !== 'string') {
    throw new Error('Cube id must be a string.');
  }
  const trimmed = cubeId.trim();
  if (!trimmed) {
    throw new Error('Cube id is required.');
  }
  const separator = trimmed.indexOf('/');
  if (separator <= 0) {
    throw new Error(
      'Cube id must use canonical owner/repo/path/to/cube.cube or local/namespace/path/to/cube.cube format.',
    );
  }
  const firstSegment = trimmed.slice(0, separator);
  if (firstSegment === 'local') {
    return parseLocalCubeId(trimmed.slice(separator + 1));
  }
  return parseGithubCubeId(trimmed);
}

/**
 * Return whether a cube id uses canonical source-qualified format.
 */
export function isCanonicalCubeId(cubeId: unknown): boolean {
  try {
    parseCanonicalCubeId(cubeId);
    return true;
  } catch (_error) {
    return false;
  }
}

/**
 * Normalize an authored cube display title without changing path semantics.
 */
export function normalizeDefaultAliasTitle(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }
  const cleaned = value.trim();
  if (!cleaned) {
    return '';
  }
  if (cleaned.includes('_') || !ASCII_LETTER_RE.test(cleaned)) {
    return cleaned;
  }
  const parts = cleaned.split(/\s+/);
  return parts
    .map((part, index) =>
      normalizeTitleToken(part, {
        index,
        isLast: index === parts.length - 1,
      }),
    )
    .join(' ');
}

/**
 * Suggest a source-relative cube filename from a display name.
 */
export function suggestCanonicalCubePath(defaultAlias: unknown): string {
  const filename = normalizeDefaultAliasTitle(defaultAlias) || 'cube';
  const withExtension = filename.toLowerCase().endsWith('.cube') ? filename : `${filename}.cube`;
  return validateCubePathSegment(withExtension);
}

/**
 * Derive a target cube id by keeping the current source and folder.
 */
export function deriveCubeIdFromDefaultAlias(cubeId: unknown, defaultAlias: unknown): string {
  const parsed = parseCanonicalCubeId(cubeId);
  const pathParts = parsed.path.split('/');
  pathParts[pathParts.length - 1] = suggestCanonicalCubePath(defaultAlias);
  return `${parsed.sourceRoot}/${pathParts.join('/')}`;
}

/**
 * Derive the product author label from canonical source identity.
 */
export function deriveSourceAuthorLabel(
  cubeId: unknown,
  { localLabel = 'local' }: { localLabel?: string } = {},
): string {
  const parsed = parseCanonicalCubeId(cubeId);
  if (parsed.sourceKind === 'github') {
    return parsed.repoRef;
  }
  return localLabel;
}

/**
 * Suggest one canonical local cube id from a display name.
 */
export function suggestLocalCubeId(defaultAlias: unknown, namespace = 'personal'): string {
  const safeNamespace = LOCAL_NAMESPACE_RE.test(namespace) ? namespace : 'personal';
  return `local/${safeNamespace}/${suggestCanonicalCubePath(defaultAlias)}`;
}

function parseGithubCubeId(remainder: string): CanonicalCubeId {
  const parts = remainder.split('/', 3);
  if (parts.length !== 3) {
    throw new Error('Cube id must use canonical owner/repo/path/to/cube.cube format.');
  }
  const owner = parts[0] ?? '';
  const repo = parts[1] ?? '';
  const relativePath = remainder.slice(owner.length + repo.length + 2);
  if (RESERVED_SOURCE_NAMES.has(owner.toLowerCase())) {
    throw new Error(`Cube id owner '${owner}' is reserved.`);
  }
  if (!OWNER_RE.test(owner)) {
    throw new Error('Cube id owner is invalid.');
  }
  if (!REPO_RE.test(repo)) {
    throw new Error('Cube id repo is invalid.');
  }
  const normalizedPath = normalizeCanonicalCubePath(relativePath);
  const cubeId = `${owner}/${repo}/${normalizedPath}`;
  return {
    sourceKind: 'github',
    owner,
    repo,
    namespace: '',
    path: normalizedPath,
    cubeId,
    sourceRoot: `${owner}/${repo}`,
    repoRef: `${owner}/${repo}`,
  };
}

function parseLocalCubeId(remainder: string): CanonicalCubeId {
  const parts = remainder.split('/', 2);
  if (parts.length !== 2) {
    throw new Error('Cube id must use canonical local/namespace/path/to/cube.cube format.');
  }
  const namespace = parts[0] ?? '';
  const relativePath = remainder.slice(namespace.length + 1);
  if (RESERVED_SOURCE_NAMES.has(namespace.toLowerCase())) {
    throw new Error(`Cube id local namespace '${namespace}' is reserved.`);
  }
  if (!LOCAL_NAMESPACE_RE.test(namespace)) {
    throw new Error('Cube id local namespace is invalid.');
  }
  const normalizedPath = normalizeCanonicalCubePath(relativePath);
  const cubeId = `local/${namespace}/${normalizedPath}`;
  return {
    sourceKind: 'local',
    owner: '',
    repo: '',
    namespace,
    path: normalizedPath,
    cubeId,
    sourceRoot: `local/${namespace}`,
    repoRef: '',
  };
}

function normalizeCanonicalCubePath(relativePath: unknown): string {
  const cleaned =
    typeof relativePath === 'string'
      ? relativePath
          .trim()
          .replace(/\\/g, '/')
          .replace(/^\/+|\/+$/g, '')
      : '';
  if (!cleaned) {
    throw new Error('Cube id path is required.');
  }
  const segments = cleaned.split('/');
  if (
    !segments.length ||
    segments.some((segment) => !segment || segment === '.' || segment === '..')
  ) {
    throw new Error('Cube id path must stay within the tracked repo.');
  }
  if (!cleaned.toLowerCase().endsWith('.cube')) {
    throw new Error("Cube id path must end in '.cube'.");
  }
  segments.forEach((segment) => validateCubePathSegment(segment));
  return segments.join('/');
}

function normalizeTitleToken(token: string, { index, isLast }: TitleTokenPosition): string {
  const wordMatch = token.match(ASCII_WORD_RE);
  if (!wordMatch) {
    return token;
  }
  const word = wordMatch[0];
  const wordLower = word.toLowerCase();
  let replacement = '';
  if (TITLE_TECH_TERMS.has(wordLower)) {
    replacement = TITLE_TECH_TERMS.get(wordLower) ?? word;
  } else if (index > 0 && !isLast && TITLE_SMALL_WORDS.has(wordLower)) {
    replacement = wordLower;
  } else if (VERSION_TOKEN_RE.test(token)) {
    return token.toLowerCase();
  } else if (hasMixedCasingBeyondFirstCharacter(word)) {
    return token;
  } else {
    replacement = `${word.slice(0, 1).toUpperCase()}${word.slice(1).toLowerCase()}`;
  }
  const start = wordMatch.index || 0;
  return `${token.slice(0, start)}${replacement}${token.slice(start + word.length)}`;
}

function hasMixedCasingBeyondFirstCharacter(word: string): boolean {
  const afterFirst = word.slice(1);
  return /[A-Z]/.test(afterFirst) && /[a-z]/.test(word);
}
