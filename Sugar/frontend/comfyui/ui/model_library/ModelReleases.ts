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
/** Validate provider-neutral release choices without exposing remote download addresses. */
import { isRecord } from '../types/common.js';

export interface ModelReleaseArtifact {
  readonly access: 'public' | 'api_key_required' | 'unknown';
  readonly fileId: string;
  readonly fileName: string;
  readonly sha256: string;
  readonly sizeBytes: number;
  readonly kind: string;
}
export interface ModelRelease {
  readonly versionId: string;
  readonly name: string;
  readonly baseModel: string;
  readonly publishedAt: string;
  readonly pageUrl: string;
  readonly files: readonly ModelReleaseArtifact[];
  readonly previewUrl: string | null;
  readonly onDisk: boolean;
  readonly installedFileName?: string | null;
  readonly inUse: boolean | null;
}

/** Reject malformed history before presenting any actionable acquisition control. */
export function decodeModelReleases(value: unknown): readonly ModelRelease[] {
  if (!isRecord(value) || value.schemaVersion !== 1 || !Array.isArray(value.versions))
    throw new Error('Invalid model release history.');
  return value.versions.map((version: unknown) => {
    if (!isRecord(version) || !Array.isArray(version.files))
      throw new Error('Invalid model release.');
    return {
      versionId: identity(version.versionId),
      name: text(version.name),
      baseModel: text(version.baseModel),
      publishedAt: text(version.publishedAt),
      pageUrl: text(version.pageUrl),
      previewUrl: preview(version.previewUrl),
      onDisk: version.onDisk === true,
      installedFileName: installedFileName(version.installedFileName),
      inUse: useState(version.inUse),
      files: version.files.map((file: unknown) => {
        if (
          !isRecord(file) ||
          typeof file.sha256 !== 'string' ||
          !/^[a-f0-9]{64}$/.test(file.sha256) ||
          typeof file.sizeBytes !== 'number' ||
          !Number.isSafeInteger(file.sizeBytes) ||
          file.sizeBytes <= 0
        )
          throw new Error('Invalid model artifact.');
        return {
          access: downloadAccess(file.access),
          fileId: identity(file.fileId),
          fileName: text(file.fileName),
          sha256: file.sha256,
          sizeBytes: file.sizeBytes,
          kind: text(file.kind),
        };
      }),
    };
  });
}

/** Preserve omitted state from older public host responses while validating authoritative flags. */
function useState(value: unknown): boolean | null {
  if (value === undefined) return null;
  if (typeof value !== 'boolean') throw new Error('Invalid current model release state.');
  return value;
}

/** Admit only the server-owned exact-version preview route, never arbitrary provider addresses. */
function preview(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (
    typeof value !== 'string' ||
    !/^\/sugarcubes\/v1\/models\/[a-f0-9]{64}\/versions\/[1-9][0-9]{0,15}\/preview$/.test(value)
  )
    throw new Error('Invalid model release preview.');
  return value;
}

/** Keep opaque provider identities bounded before using them in acquisition requests. */
function identity(value: unknown): string {
  if (typeof value !== 'string' || !/^[1-9][0-9]{0,15}$/.test(value))
    throw new Error('Invalid model release identity.');
  return value;
}
/** Preserve labels literally without coercing unknown payload values. */
function text(value: unknown): string {
  if (typeof value !== 'string') throw new Error('Invalid model release text.');
  return value;
}

/** Accept only a relative host filename supplied by the local catalog boundary. */
function installedFileName(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (
    typeof value !== 'string' ||
    !value ||
    value.length > 1024 ||
    /^[\\/]/.test(value) ||
    value.includes(':') ||
    /\p{Cc}/u.test(value) ||
    value
      .replaceAll('\\', '/')
      .split('/')
      .some((part) => part === '..' || part === '.')
  )
    throw new Error('Invalid installed model filename.');
  return value;
}

/** Keep omitted host evidence unknown so an older response cannot grant download access. */
function downloadAccess(value: unknown): ModelReleaseArtifact['access'] {
  if (value === undefined) return 'unknown';
  if (value === 'public' || value === 'api_key_required' || value === 'unknown') return value;
  throw new Error('Invalid model download access.');
}
