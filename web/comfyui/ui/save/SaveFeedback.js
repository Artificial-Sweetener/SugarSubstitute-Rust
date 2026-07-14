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
/** Normalize save errors and user-facing summaries. */

/** Represent one structured backend export failure. */
export class SugarCubeExportError extends Error {
  constructor(message, detail = '', violations = undefined) {
    super(message);
    this.detail = detail;
    this.violations = violations;
  }

  static from(error) {
    if (!error) {
      return new SugarCubeExportError('Export failed');
    }
    if (error instanceof SugarCubeExportError) {
      return error;
    }
    if (typeof error === 'object') {
      return new SugarCubeExportError(
        error.message || 'Export failed',
        error.detail || '',
        error.violations,
      );
    }
    return new SugarCubeExportError(String(error));
  }
}

/** Format one saved artifact and its optional Git result. */
export function formatSaveSummaryEntry(entry) {
  const prefix = entry?.committed ? 'saved and committed' : 'saved only';
  const defaultAlias = entry?.default_alias || 'SugarCube';
  const path = typeof entry?.path === 'string' ? entry.path : '';
  const commitSuffix =
    entry?.committed && entry?.commit_short_sha
      ? ` (${entry.commit_short_sha}: ${entry.commit_message || 'committed'})`
      : entry?.commit_error
        ? ` (commit failed: ${entry.commit_error})`
        : '';
  return `${prefix}: ${defaultAlias} -> ${path}${commitSuffix}`;
}

/** Extract a readable detail string from one backend error payload. */
export function buildErrorDetail(errorPayload) {
  if (!errorPayload) {
    return '';
  }
  if (typeof errorPayload.detail === 'string' && errorPayload.detail) {
    return errorPayload.detail;
  }
  if (errorPayload.details && typeof errorPayload.details === 'object') {
    try {
      return JSON.stringify(errorPayload.details);
    } catch (_error) {
      return '';
    }
  }
  return '';
}

/** Format graph-boundary violations as endpoint pairs. */
export function formatViolations(violations) {
  if (!Array.isArray(violations) || violations.length === 0) {
    return '';
  }
  return violations
    .map((entry) => `${formatEndpoint(entry?.from)} -> ${formatEndpoint(entry?.to)}`)
    .join('\n');
}

/** Format one graph endpoint without trusting optional backend fields. */
export function formatEndpoint(endpoint) {
  if (!endpoint || typeof endpoint !== 'object') {
    return '<unknown>';
  }
  const parts = [];
  if (endpoint.title) parts.push(endpoint.title);
  if (endpoint.cube) parts.push(`[${endpoint.cube}]`);
  if (endpoint.port !== undefined) parts.push(`(${endpoint.port})`);
  if (!parts.length && endpoint.id) parts.push(String(endpoint.id));
  return parts.join(' ') || '<unknown>';
}
