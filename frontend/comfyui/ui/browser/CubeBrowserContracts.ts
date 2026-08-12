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
/** Define the host and application boundaries used by the Cube browser owners. */

import { isRecord } from '../types/common.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { CubeLibraryEntry } from './CubeBrowserStore.js';

export interface BrowserAdapter {
  getDocument?(): Document | null;
  getWindow?(): Window | null;
  getFetch?(): typeof fetch | null;
  getConsole?(): Console | null;
  getLiteGraph?(): LiteGraphHost | null;
  getNodeRenderer?(): 'litegraph' | 'vue';
}

export interface BrowserApi {
  list(options?: RequestInit): Promise<ApiJsonResult>;
  listRevisions(cubeId: string): Promise<ApiJsonResult>;
  delete(reference?: string | UnknownRecord | null): Promise<ApiJsonResult>;
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  rename(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  updateMetadata(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}

export interface BrowserStorage {
  readList(key: string): string[];
  writeList(key: string, values: readonly string[]): void;
  readSet(key: string): Set<string>;
  writeSet(key: string, values: ReadonlySet<string>): void;
}

export interface BrowserToast {
  push(severity: string, summary: string, detail: string): void;
}

export interface BrowserImportResult extends UnknownRecord {
  success?: boolean;
}

export interface BusyImportOptions {
  dropOrigin: Vec2;
  setBusy(busy: boolean): void;
}

export interface BrowserActions {
  computeDropOrigin?(): Vec2;
  emitProximityLog?(name: string, detail: UnknownRecord): void;
  onCubesUpdated?(cubes: CubeLibraryEntry[]): void;
  openConfirmDialog?(options: {
    title: string;
    message: string | string[];
    confirmLabel: string;
  }): Promise<boolean> | boolean;
  reconcileCubeIdentity?(options: {
    previousCubeId: string;
    cubeId: string;
    defaultAlias: string;
  }): void;
  importCubeByName?(cubeId: string, options: BusyImportOptions): Promise<BrowserImportResult>;
  importCubeRevision?(
    cubeId: string,
    revisionRef: string,
    options: BusyImportOptions,
  ): Promise<BrowserImportResult>;
  promoteCube?(cube: CubeLibraryEntry): Promise<unknown> | unknown;
  startCubePlacement?(
    cubeId: string,
    options: {
      closeBrowser: boolean;
      defaultAlias: string;
      revisionRef: string;
      version: string;
    },
  ): void;
}

const unavailableApiCall = async (): Promise<ApiJsonResult> => {
  throw new Error('Cube library API unavailable');
};

const UNAVAILABLE_BROWSER_API: BrowserApi = {
  list: unavailableApiCall,
  listRevisions: unavailableApiCall,
  delete: unavailableApiCall,
  load: unavailableApiCall,
  loadRevision: unavailableApiCall,
  rename: unavailableApiCall,
  updateMetadata: unavailableApiCall,
};

/** Bind every optional API operation to one complete browser API boundary. */
export function resolveBrowserApi(api: Partial<BrowserApi> | null | undefined): BrowserApi {
  return {
    list: api?.list?.bind(api) ?? UNAVAILABLE_BROWSER_API.list,
    listRevisions: api?.listRevisions?.bind(api) ?? UNAVAILABLE_BROWSER_API.listRevisions,
    delete: api?.delete?.bind(api) ?? UNAVAILABLE_BROWSER_API.delete,
    load: api?.load?.bind(api) ?? UNAVAILABLE_BROWSER_API.load,
    loadRevision: api?.loadRevision?.bind(api) ?? UNAVAILABLE_BROWSER_API.loadRevision,
    rename: api?.rename?.bind(api) ?? UNAVAILABLE_BROWSER_API.rename,
    updateMetadata: api?.updateMetadata?.bind(api) ?? UNAVAILABLE_BROWSER_API.updateMetadata,
  };
}

/** Return whether an API payload contains a structured error. */
export function hasApiError(data: UnknownRecord): boolean {
  return isRecord(data.error);
}

/** Read the most specific browser API error message available. */
export function readApiErrorMessage(data: UnknownRecord, fallback: string): string {
  const error = isRecord(data.error) ? data.error : null;
  return (typeof error?.message === 'string' && error.message.trim()) || fallback;
}

/** Normalize an unknown thrown value for browser feedback. */
export function readErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
