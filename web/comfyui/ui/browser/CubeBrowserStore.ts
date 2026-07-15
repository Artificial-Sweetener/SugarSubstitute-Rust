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
 * Own the SugarCubes cube browser layer in `web/comfyui/ui/browser/CubeBrowserStore.js`.
 */

import { isRecord } from '../types/common.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';

export interface CubeLibraryEntry extends UnknownRecord {
  cube_id?: string;
  default_alias?: string;
  name?: string;
  version?: string;
  author?: string;
  author_url?: string;
  target_model?: string;
  supported_models?: string[];
  tags?: string[];
  mtime?: string | number;
  is_writable?: boolean;
  display_name?: string;
  description?: string;
  namespace?: string;
  owner?: string;
  repo?: string;
  relative_path?: string;
  write_block_reason?: string;
  icon?: UnknownRecord;
  source?: UnknownRecord;
  metadata?: UnknownRecord;
  lineage?: UnknownRecord | null;
}

export interface CubeAuthorGroup {
  key: string;
  label: string;
  authorLabel: string;
  cubes: CubeLibraryEntry[];
}

export interface CubeRevision extends UnknownRecord {
  revision_ref?: string;
  version?: string;
}

export interface CubeVersionOption extends UnknownRecord {
  label: string;
  value: string;
  revisionRef: string;
  current: boolean;
  raw: CubeRevision | null;
}

export interface CubeEditDraft extends UnknownRecord {
  name: string;
  original_name: string;
  description: string;
  current_cube_id: string;
  derived_cube_id: string;
  cube_id: string;
  version: string;
  author_url: string;
  tags: string[];
  target_model: string;
  supported_models: string[];
}

export interface CubeBrowserState {
  initialized: boolean;
  loading: boolean;
  busy: boolean;
  error: string | null;
  cubes: CubeLibraryEntry[];
  filtered: CubeLibraryEntry[];
  grouped: CubeAuthorGroup[];
  selected: string | null;
  favorites: Set<string>;
  recents: string[];
  modelOptions: string[];
  searchQuery: string;
  editing: boolean;
  editDraft: CubeEditDraft | null;
  dirtyCubeIds: Set<string>;
  lastFetched: number;
  authorGroupsOpen: Set<string>;
  authorGroupsTouched: boolean;
  dropOrigin: Vec2;
  revisions: CubeRevision[];
  revisionsLoading: boolean;
  selectedRevision: string;
  revisionsCubeId: string | null;
  versionOptions: CubeVersionOption[];
  selectedVersion: string;
  versionError: string | null;
}

function readRecords<T extends UnknownRecord>(value: unknown): T[] {
  return Array.isArray(value) ? (value.filter(isRecord) as T[]) : [];
}

function readStrings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string')
    : [];
}

/**
 * Coordinate cube browser store behavior for the SugarCubes UI.
 */
export class CubeBrowserStore {
  state: CubeBrowserState;

  constructor() {
    this.state = {
      initialized: false,
      loading: false,
      busy: false,
      error: null,
      cubes: [],
      filtered: [],
      grouped: [],
      selected: null,
      favorites: new Set(),
      recents: [],
      modelOptions: [],
      searchQuery: '',
      editing: false,
      editDraft: null,
      dirtyCubeIds: new Set(),
      lastFetched: 0,
      authorGroupsOpen: new Set(),
      authorGroupsTouched: false,
      dropOrigin: [0, 0],
      revisions: [],
      revisionsLoading: false,
      selectedRevision: 'WORKTREE',
      revisionsCubeId: null,
      versionOptions: [],
      selectedVersion: '',
      versionError: null,
    };
  }

  hydrate(next: Partial<CubeBrowserState>): void {
    this.state = { ...this.state, ...next };
  }

  update(patch: Partial<CubeBrowserState>): void {
    this.state = { ...this.state, ...patch };
  }

  setInitialized(value: unknown): void {
    this.update({ initialized: Boolean(value) });
  }

  setLoading(value: unknown): void {
    this.update({ loading: Boolean(value) });
  }

  setBusy(value: unknown): void {
    this.update({ busy: Boolean(value) });
  }

  setError(message: unknown): void {
    this.update({ error: typeof message === 'string' && message ? message : null });
  }

  setCubes(cubes: unknown): void {
    this.update({ cubes: readRecords<CubeLibraryEntry>(cubes) });
  }

  setFiltered(filtered: unknown): void {
    this.update({ filtered: readRecords<CubeLibraryEntry>(filtered) });
  }

  setGrouped(groups: CubeAuthorGroup[] | null | undefined): void {
    this.update({ grouped: Array.isArray(groups) ? groups : [] });
  }

  setSelected(name: unknown): void {
    this.update({ selected: typeof name === 'string' && name ? name : null });
  }

  setFavorites(favorites: Iterable<string> | null | undefined): void {
    this.update({ favorites: new Set(favorites || []) });
  }

  setRecents(recents: unknown): void {
    this.update({ recents: readStrings(recents) });
  }

  setModelOptions(options: unknown): void {
    this.update({ modelOptions: readStrings(options) });
  }

  setSearchQuery(query: unknown): void {
    this.update({ searchQuery: typeof query === 'string' ? query : '' });
  }

  setEditing(editing: unknown, draft: unknown = null): void {
    this.update({
      editing: Boolean(editing),
      editDraft: isRecord(draft) ? (draft as CubeEditDraft) : null,
    });
  }

  setDirtyCubeIds(dirtyCubeIds: Set<string> | null | undefined): void {
    this.update({ dirtyCubeIds: dirtyCubeIds instanceof Set ? dirtyCubeIds : new Set() });
  }

  setLastFetched(timestamp: unknown): void {
    this.update({ lastFetched: Number(timestamp) || 0 });
  }

  setAuthorGroupsOpen(groups: Set<string> | null | undefined): void {
    this.update({ authorGroupsOpen: groups instanceof Set ? groups : new Set() });
  }

  setAuthorGroupsTouched(value: unknown): void {
    this.update({ authorGroupsTouched: Boolean(value) });
  }

  setDropOrigin(origin: unknown): void {
    const fallback: Vec2 = [0, 0];
    if (!Array.isArray(origin) || origin.length < 2) {
      this.update({ dropOrigin: fallback });
      return;
    }
    const x = Number(origin[0]);
    const y = Number(origin[1]);
    this.update({ dropOrigin: [Number.isFinite(x) ? x : 0, Number.isFinite(y) ? y : 0] });
  }

  setRevisions(revisions: unknown, cubeId: unknown = null): void {
    this.update({
      revisions: readRecords<CubeRevision>(revisions),
      revisionsCubeId: typeof cubeId === 'string' && cubeId ? cubeId : null,
    });
  }

  setRevisionsLoading(value: unknown): void {
    this.update({ revisionsLoading: Boolean(value) });
  }

  setSelectedRevision(value: unknown): void {
    const selectedRevision = typeof value === 'string' && value.trim() ? value.trim() : 'WORKTREE';
    this.update({ selectedRevision });
  }

  setVersionOptions(options: unknown): void {
    this.update({ versionOptions: readRecords<CubeVersionOption>(options) });
  }

  setSelectedVersion(value: unknown): void {
    const selectedVersion = typeof value === 'string' && value.trim() ? value.trim() : '';
    this.update({ selectedVersion });
  }

  setVersionError(message: unknown): void {
    this.update({ versionError: typeof message === 'string' && message ? message : null });
  }

  resetVersionState(): void {
    this.update({
      versionOptions: [],
      selectedVersion: '',
      versionError: null,
    });
  }
}
