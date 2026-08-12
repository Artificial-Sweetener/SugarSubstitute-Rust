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
/** Own Cube revision retrieval and version-selection policy. */

import {
  CURRENT_REVISION_REF,
  normalizeCubeVersion,
  normalizeRevisionRef,
} from '../core/CubeDefinitionKey.js';
import { projectCubeVersionOptions } from '../cube/version/CubeVersionOptionProjection.js';
import {
  hasApiError,
  readApiErrorMessage,
  readErrorMessage,
  type BrowserApi,
  type BrowserToast,
} from './CubeBrowserContracts.js';
import type { CubeBrowserStore, CubeLibraryEntry, CubeVersionOption } from './CubeBrowserStore.js';

interface VersionScore {
  category: number;
  distance: number;
  index: number;
}

interface RevisionOptions {
  api: BrowserApi;
  store: CubeBrowserStore;
  toast: BrowserToast | null;
  getCube(cubeKey: unknown): CubeLibraryEntry | null;
  render(): void;
  requestPreview(cubeKey: string | null): void;
}

/** Coordinate revision history and the browser's version-combobox state. */
export class CubeBrowserRevisionCoordinator {
  constructor(private readonly options: RevisionOptions) {}

  /** Reset revision state and request history for the selected Cube. */
  selectCube(selected: CubeLibraryEntry | null): void {
    const { store } = this.options;
    store.setSelectedRevision(CURRENT_REVISION_REF);
    store.setRevisions([], null);
    store.setRevisionsLoading(false);
    store.resetVersionState();
    const fallback = projectCubeVersionOptions([], selected?.version)[0];
    if (fallback) {
      store.setVersionOptions([fallback]);
      store.setSelectedVersion(fallback.value);
    }
  }

  /** Load the revision catalog for one selected Cube. */
  async requestRevisions(cubeKey: unknown): Promise<void> {
    const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
    const selected = key ? this.options.getCube(key) : null;
    const cubeId = selected?.cube_id || '';
    if (!cubeId) {
      this.selectCube(null);
      this.options.render();
      return;
    }
    const { store } = this.options;
    store.setRevisionsLoading(true);
    this.options.render();
    try {
      const { response, data } = await this.options.api.listRevisions(cubeId);
      if (!response.ok || hasApiError(data)) {
        this.reportFailure(
          cubeId,
          readApiErrorMessage(data, response.statusText || 'Failed to load revisions'),
        );
        return;
      }
      if (this.options.getCube(store.state.selected)?.cube_id !== cubeId) {
        return;
      }
      const revisions = Array.isArray(data?.revisions) ? data.revisions : [];
      const versionRevisions = Array.isArray(data?.version_revisions)
        ? data.version_revisions
        : revisions;
      const versionOptions = projectCubeVersionOptions(versionRevisions, selected?.version);
      store.setRevisions(revisions, cubeId);
      store.setVersionOptions(versionOptions);
      store.setVersionError(null);
      const activeRevision = versionOptions.some(
        (entry) => entry?.revisionRef === store.state.selectedRevision,
      )
        ? store.state.selectedRevision
        : CURRENT_REVISION_REF;
      store.setSelectedRevision(activeRevision);
      const activeOption =
        versionOptions.find((entry) => entry.revisionRef === activeRevision) ||
        versionOptions[0] ||
        null;
      if (activeOption) {
        store.setSelectedVersion(activeOption.value);
      }
    } catch (error) {
      this.reportFailure(cubeId, readErrorMessage(error));
    } finally {
      store.setRevisionsLoading(false);
      this.options.render();
      this.options.requestPreview(store.state.selected);
    }
  }

  /** Select one revision and synchronize its displayed version. */
  selectRevision(revisionRef: unknown): void {
    const normalized = normalizeRevisionRef(revisionRef);
    const { store } = this.options;
    store.setSelectedRevision(normalized);
    const option = store.state.versionOptions.find((entry) => entry.revisionRef === normalized);
    if (option) {
      store.setSelectedVersion(option.value);
    }
    this.options.render();
    this.options.requestPreview(store.state.selected);
  }

  /** Select one exact displayed version. */
  selectVersion(version: unknown): void {
    const normalized = normalizeCubeVersion(version);
    const option = this.options.store.state.versionOptions.find(
      (entry) => entry.value === normalized,
    );
    if (option) {
      this.selectRevision(option.revisionRef);
    }
  }

  /** Resolve free-form version input to the closest known revision. */
  commitVersionInput(version: unknown): CubeVersionOption | null {
    const { store } = this.options;
    const options = store.state.versionOptions;
    const fallback =
      options.find((entry) => entry.value === store.state.selectedVersion) ||
      options.find((entry) => entry.revisionRef === store.state.selectedRevision) ||
      options[0] ||
      null;
    const option = this.findClosestVersionOption(version, options) || fallback;
    if (option) {
      this.selectRevision(option.revisionRef);
    }
    return option;
  }

  /** Find the best exact, prefix, substring, or semantic version match. */
  findClosestVersionOption(
    typedValue: unknown,
    options: readonly CubeVersionOption[] = [],
  ): CubeVersionOption | null {
    const scored = options
      .map((option, index) => ({ option, score: this.score(typedValue, option, index) }))
      .filter(
        (entry): entry is { option: CubeVersionOption; score: VersionScore } =>
          entry.score !== null,
      )
      .sort((left, right) =>
        left.score.category !== right.score.category
          ? left.score.category - right.score.category
          : left.score.distance !== right.score.distance
            ? left.score.distance - right.score.distance
            : left.score.index - right.score.index,
      );
    return scored[0]?.option ?? null;
  }

  /** Return the version option represented by current browser state. */
  getSelectedVersionOption(): CubeVersionOption | null {
    const { store } = this.options;
    return (
      store.state.versionOptions.find(
        (entry) => entry.revisionRef === store.state.selectedRevision,
      ) ||
      store.state.versionOptions.find((entry) => entry.value === store.state.selectedVersion) ||
      null
    );
  }

  private reportFailure(cubeId: string, message: string): void {
    const { store } = this.options;
    store.setRevisions([], cubeId);
    store.setSelectedRevision(CURRENT_REVISION_REF);
    store.setVersionError(message);
    this.options.toast?.push('warn', 'Revision history unavailable', message);
  }

  private score(
    typedValue: unknown,
    option: CubeVersionOption,
    index: number,
  ): VersionScore | null {
    const typed = normalizeCubeVersion(typedValue).toLowerCase();
    const value = normalizeCubeVersion(option?.value).toLowerCase();
    if (!typed || !value) return null;
    if (typed === value) return { category: 0, distance: 0, index };
    if (value.startsWith(typed)) {
      return { category: 1, distance: value.length - typed.length, index };
    }
    const substringIndex = value.indexOf(typed);
    if (substringIndex >= 0) {
      return { category: 2, distance: substringIndex + value.length - typed.length, index };
    }
    const typedParts = this.parseSemver(typed);
    const valueParts = this.parseSemver(value);
    if (!typedParts || !valueParts) return null;
    return {
      category: 3,
      distance:
        Math.abs(valueParts[0] - typedParts[0]) * 1000000 +
        Math.abs(valueParts[1] - typedParts[1]) * 1000 +
        Math.abs(valueParts[2] - typedParts[2]),
      index,
    };
  }

  private parseSemver(value: string): [number, number, number] | null {
    const match = /^(\d+)(?:\.(\d+))?(?:\.(\d+))?$/.exec(value);
    return match ? [Number(match[1] || 0), Number(match[2] || 0), Number(match[3] || 0)] : null;
  }
}
