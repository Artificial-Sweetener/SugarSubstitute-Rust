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
 * Own current and historical cube import command orchestration.
 */

import { readVector2 } from '../graph/VectorUtils.js';
import { isRecord } from '../types/common.js';
import {
  buildImportSummary,
  prepareGraphInsertionPayload,
  readImportPayload,
} from './PlacementPayload.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { ImportPayload } from './PlacementPayload.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { GraphId } from '../types/graph.js';

type ToastSeverity = 'success' | 'info' | 'warn' | 'error';

export interface ImportResult {
  success: boolean;
  summary: string;
  message: string;
  warnings: string[];
  missingTypes: string[];
  nodesAdded: number;
  markersAdded: number;
  connectionsMade: number;
  primaryNodeId: GraphId | null;
  bounds: { minX: number; minY: number; maxX: number; maxY: number } | null;
}

export interface ImportOptions {
  dropOrigin?: Vec2;
  focus?: boolean;
  setBusy?(busy: boolean): void;
  button?: { enabled: boolean; element: HTMLElement };
}

export interface CubeImportCommandResult extends UnknownRecord {
  success: boolean;
  reason?: string;
}

interface ImportApi {
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

interface CubeImportCommandDependencies {
  api: ImportApi;
  applyPreparedImport(
    payload: ImportPayload,
    options: { instanceAlias: string; dropOrigin: Vec2 },
  ): Promise<ImportResult | null>;
  computeDropOrigin(): Vec2;
  focusImportedNode(result: ImportResult): void;
  persistLastCubeId(cubeId: string): void;
  pushToast(severity: ToastSeverity, summary: string, detail: string): void;
  readErrorMessage(error: unknown): string;
}

interface ImportRequest {
  cubeId: string;
  revisionRef: string | null;
  options: ImportOptions;
}

/** Coordinate cube import requests and their user-visible outcomes. */
export class CubeImportCommandService {
  private readonly dependencies: CubeImportCommandDependencies;

  constructor(dependencies: CubeImportCommandDependencies) {
    this.dependencies = dependencies;
  }

  /** Import the current working-tree definition for one cube. */
  importCurrent(rawCubeId: unknown, options: ImportOptions = {}): Promise<CubeImportCommandResult> {
    const cubeId = readRequiredString(rawCubeId);
    if (!cubeId) return Promise.resolve({ success: false, reason: 'empty' });
    return this.execute({ cubeId, revisionRef: null, options });
  }

  /** Import one historical cube revision, routing WORKTREE through the current endpoint. */
  importRevision(
    rawCubeId: unknown,
    rawRevisionRef: unknown,
    options: ImportOptions = {},
  ): Promise<CubeImportCommandResult> {
    const cubeId = readRequiredString(rawCubeId);
    const revisionRef = readRequiredString(rawRevisionRef);
    if (!cubeId || !revisionRef) return Promise.resolve({ success: false, reason: 'empty' });
    if (revisionRef === 'WORKTREE') return this.importCurrent(cubeId, options);
    return this.execute({ cubeId, revisionRef, options });
  }

  /** Execute one normalized current or historical import request. */
  private async execute({
    cubeId,
    revisionRef,
    options,
  }: ImportRequest): Promise<CubeImportCommandResult> {
    const historical = revisionRef !== null;
    const failureTitle = historical ? 'Revision import failed' : 'Import failed';
    const dropOrigin = resolveDropOrigin(options.dropOrigin, this.dependencies.computeDropOrigin);
    const setBusy = resolveBusySetter(options);
    this.dependencies.persistLastCubeId(cubeId);
    setBusy(true);

    try {
      const body = historical
        ? { cube_id: cubeId, revision_ref: revisionRef, origin: toOriginPayload(dropOrigin) }
        : { cube_id: cubeId, origin: toOriginPayload(dropOrigin) };
      const request = historical
        ? this.dependencies.api.loadRevision.bind(this.dependencies.api)
        : this.dependencies.api.load.bind(this.dependencies.api);
      const { response, data } = await request(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
      });
      const errorPayload = isRecord(data.error) ? data.error : null;
      if (!response.ok || errorPayload) {
        const message =
          (typeof errorPayload?.message === 'string' && errorPayload.message) ||
          response.statusText ||
          failureTitle;
        const detail =
          typeof errorPayload?.detail === 'string' && errorPayload.detail
            ? errorPayload.detail
            : '';
        this.dependencies.pushToast('error', message, detail);
        return {
          success: false,
          reason: 'error',
          message,
          detail,
          status: response.status,
          payload: data,
        };
      }

      const decodedData = readImportPayload(data);
      if (!decodedData) {
        throw new Error(
          historical
            ? 'Revision response is not a valid SugarCubes payload.'
            : 'Import response is not a valid SugarCubes payload.',
        );
      }
      const targetOrigin = readVector2(decodedData.layout?.origin, dropOrigin[0], dropOrigin[1]);
      const preparedData = prepareGraphInsertionPayload(decodedData, {
        targetOrigin,
        remapInstanceIds: true,
      });
      if (!preparedData) {
        throw new Error(
          historical
            ? 'Revision payload preparation failed.'
            : 'Import payload preparation failed.',
        );
      }
      const importResult = await this.dependencies.applyPreparedImport(preparedData, {
        instanceAlias: readImportAlias(preparedData, cubeId),
        dropOrigin,
      });
      const backendWarnings = readWarningMessages(data.warnings);
      if (backendWarnings.length) {
        this.dependencies.pushToast(
          'warn',
          historical ? 'SugarCube revision import warnings' : 'SugarCube import warnings',
          backendWarnings.join('\n'),
        );
      }

      const frontendWarnings = Array.isArray(importResult?.warnings)
        ? importResult.warnings.filter(Boolean)
        : [];
      if (Array.isArray(importResult?.missingTypes) && importResult.missingTypes.length) {
        frontendWarnings.push(`Missing node types: ${importResult.missingTypes.join(', ')}`);
      }
      if (importResult?.message && importResult.success)
        frontendWarnings.push(importResult.message);
      if (frontendWarnings.length) {
        this.dependencies.pushToast(
          'warn',
          historical ? 'SugarCube revision import notes' : 'SugarCube import notes',
          frontendWarnings.join('\n'),
        );
      }

      const summary = importResult?.summary ?? buildImportSummary(preparedData);
      if (!importResult?.success) {
        this.dependencies.pushToast(
          'warn',
          `SugarCube ${cubeId}${historical ? ' revision' : ''} import incomplete`,
          importResult?.message || summary,
        );
      } else {
        this.dependencies.pushToast(
          'success',
          `Imported ${cubeId}${historical ? ' revision' : ''}`,
          summary,
        );
        this.dependencies.focusImportedNode(importResult);
      }

      return {
        success: Boolean(importResult?.success),
        cubeId,
        ...(revisionRef ? { revisionRef } : {}),
        summary,
        backendWarnings,
        frontendWarnings,
        response: data,
        result: importResult,
      };
    } catch (error: unknown) {
      const message = this.dependencies.readErrorMessage(error);
      this.dependencies.pushToast('error', failureTitle, message);
      return { success: false, reason: 'exception', message, error };
    } finally {
      setBusy(false);
    }
  }
}

/** Read one required command string from an untrusted boundary. */
function readRequiredString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

/** Resolve the requested placement origin or defer to the host canvas. */
function resolveDropOrigin(dropOrigin: Vec2 | undefined, fallback: () => Vec2): Vec2 {
  return Array.isArray(dropOrigin) && dropOrigin.length === 2 ? dropOrigin : fallback();
}

/** Convert a graph point to the backend request shape. */
function toOriginPayload(origin: Vec2): { x: number; y: number } {
  return { x: origin[0], y: origin[1] };
}

/** Resolve command busy-state ownership from the caller's UI boundary. */
function resolveBusySetter(options: ImportOptions): (busy: boolean) => void {
  if (typeof options.setBusy === 'function') return options.setBusy;
  return (busy) => {
    if (!options.button) return;
    options.button.enabled = !busy;
    options.button.element.classList.toggle('sugarcubes-import--busy', Boolean(busy));
  };
}

/** Read the display alias embedded in a prepared cube payload. */
function readImportAlias(payload: ImportPayload, fallback: string): string {
  const cube = payload.cube ?? {};
  if (typeof cube.default_alias === 'string' && cube.default_alias.trim()) {
    return cube.default_alias.trim();
  }
  if (typeof cube.display_name === 'string' && cube.display_name.trim()) {
    return cube.display_name.trim();
  }
  return fallback;
}

/** Read user-visible warning strings from an untrusted backend payload. */
function readWarningMessages(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((warning): warning is string => typeof warning === 'string' && Boolean(warning))
    : [];
}
