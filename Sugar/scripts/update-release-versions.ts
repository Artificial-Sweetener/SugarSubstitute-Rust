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
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

interface PackageMetadata {
  version: string;
  packages?: Record<string, { version?: string }>;
}

function isPackageMetadata(value: unknown): value is PackageMetadata {
  return typeof value === 'object' && value !== null && 'version' in value;
}

/**
 * Write a package-style JSON version field.
 *
 * @param {URL} filePath File to update.
 * @param {string} nextVersion Semantic release version.
 */
export function writeJsonVersion(filePath: URL, nextVersion: string): void {
  const metadata: unknown = JSON.parse(readFileSync(filePath, 'utf8'));
  if (!isPackageMetadata(metadata)) {
    throw new Error(`Expected package metadata in ${filePath.pathname}.`);
  }
  metadata.version = nextVersion;

  const rootPackage = metadata.packages?.[''];
  if (rootPackage) {
    rootPackage.version = nextVersion;
  }

  writeFileSync(filePath, `${JSON.stringify(metadata, null, 2)}\n`, 'utf8');
}

/**
 * Replace a single version field in a structured text file.
 *
 * @param {URL} filePath File to update.
 * @param {RegExp} pattern Version field matcher.
 * @param {string} replacement Replacement line.
 */
export function replaceVersionField(filePath: URL, pattern: RegExp, replacement: string): void {
  const originalText = readFileSync(filePath, 'utf8');

  if (!pattern.test(originalText)) {
    throw new Error(`Could not find a version field in ${filePath.pathname}.`);
  }

  const updatedText = originalText.replace(pattern, replacement);
  writeFileSync(filePath, updatedText, 'utf8');
}

/**
 * Synchronize release metadata files for the next version.
 *
 * @param {URL} projectRoot Repository root URL.
 * @param {string} nextVersion Semantic release version.
 */
export function updateReleaseVersions(projectRoot: URL, nextVersion: string): void {
  writeJsonVersion(new URL('package.json', projectRoot), nextVersion);
  writeJsonVersion(new URL('package-lock.json', projectRoot), nextVersion);

  replaceVersionField(
    new URL('pyproject.toml', projectRoot),
    /^version = "[^"]+"\r?$/m,
    `version = "${nextVersion}"`,
  );

  replaceVersionField(
    new URL('__init__.py', projectRoot),
    /^__version__ = "[^"]+"\r?$/m,
    `__version__ = "${nextVersion}"`,
  );
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : '';

if (invokedPath === fileURLToPath(import.meta.url)) {
  const nextVersion = process.argv[2];

  if (!nextVersion) {
    throw new Error('Expected the next release version as the first argument.');
  }

  updateReleaseVersions(new URL('../', import.meta.url), nextVersion);
}
