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
/** Define complete cart planning and exact native snapshot boundaries without host dependencies. */
import type { CubeGraphAnalysis, CubeGraphSnapshot } from './CubeGraphModels.js';

/** Retain an exact current workflow instance in a complete cart plan. */
export interface ExistingCubeGraphStackEntry {
  readonly source: 'existing';
  readonly instanceId: string;
}

/** Plan one fresh stable instance from an opening catalog artifact snapshot. */
export interface NewCubeGraphStackEntry {
  readonly source: 'new';
  readonly instanceId: string;
  readonly catalogKey: string;
  readonly alias: string;
  readonly document: Readonly<Record<string, unknown>>;
  readonly size: readonly [number, number];
}

/** Express the complete desired stack as existing and newly staged instance references. */
export type CubeGraphStackEntry = ExistingCubeGraphStackEntry | NewCubeGraphStackEntry;

/** Plan a complete cart through the backend's canonical graph policy owners. */
export interface CubeGraphStackGateway {
  applyStack(
    snapshot: CubeGraphSnapshot,
    entries: readonly CubeGraphStackEntry[],
  ): Promise<CubeGraphAnalysis>;
}

/** Share one complete native snapshot owner across reorder and cart operations. */
export interface CubeGraphSnapshotSource {
  readonly available: boolean;
  snapshot(): CubeGraphSnapshot;
}

/** Commit a complete canonical cart plan atomically while retaining existing native objects. */
export interface CubeGraphStackCommit {
  commit(
    snapshot: CubeGraphSnapshot,
    entries: readonly CubeGraphStackEntry[],
    result: CubeGraphAnalysis,
  ): void;
}
