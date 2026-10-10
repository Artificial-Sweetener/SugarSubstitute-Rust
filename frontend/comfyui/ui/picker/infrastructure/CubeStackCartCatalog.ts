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
/** Capture immutable catalog generations while preserving staged payloads and row identities. */
import type { ComfyCubeRuntime } from '../../cube/ComfyCubeRuntime.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { ImportPayload } from '../../import/PlacementPayload.js';
import type { CubeStackArtifact } from '../domain/CubeStackArtifact.js';
import type { CubePickerCatalogRegistry } from '../CubePickerCatalogRegistry.js';
import type { CubePickerDescriptor } from '../CubePickerDescriptor.js';
import type { CubeStackLibraryRow } from '../CubeStackCartView.js';
import { cubeStackCartRow } from '../CubeStackCartRow.js';

/** Keep each completed generation isolated until its owning session accepts it. */
export interface CubeStackCatalogGeneration {
  readonly artifacts: readonly CubeStackArtifact[];
  readonly library: readonly CubeStackLibraryRow[];
  readonly payloads: ReadonlyMap<string, CapturedCubePlacement>;
}

/** Retain an exact catalog revision until the selected artifact needs mutable placement state. */
export interface CapturedCubePlacement {
  readonly descriptor: CubePickerDescriptor;
  readonly payload: ImportPayload;
}

/** Preserve presentation and geometry identity for an unchanged artifact within a cart lifetime. */
interface CapturedCatalogEntry {
  readonly fingerprint: string;
  readonly artifact: CubeStackArtifact;
  readonly row: CubeStackLibraryRow;
  readonly placement: CapturedCubePlacement;
}

/** Own the external registry-to-captured-artifact boundary independently of modal lifetimes. */
export class CubeStackCartCatalog {
  readonly #registry: Pick<CubePickerCatalogRegistry, 'entries' | 'capturedPayload'>;
  #entries = new Map<string, CapturedCatalogEntry>();
  /** Bind the validated artifact owner without preparing native placement state. */
  constructor(registry: Pick<CubePickerCatalogRegistry, 'entries' | 'capturedPayload'>) {
    this.#registry = registry;
  }
  /** Isolate one complete catalog without changing the live graph or registering definitions. */
  capture(
    construction: Pick<ComfyCubeRuntime['construction'], 'previewSize'>,
    generation: number,
  ): CubeStackCatalogGeneration {
    const artifacts: CubeStackArtifact[] = [];
    const library: CubeStackLibraryRow[] = [];
    const payloads = new Map<string, CapturedCubePlacement>();
    const nextEntries = new Map<string, CapturedCatalogEntry>();
    for (const { type, descriptor } of this.#registry.entries()) {
      const cached = this.#registry.capturedPayload(type);
      if (!cached) throw new Error('Cube cart catalog changed during capture.');
      const fingerprint = JSON.stringify(descriptor);
      const previous = this.#entries.get(type);
      if (descriptor.contentHash && previous?.fingerprint === fingerprint) {
        artifacts.push(previous.artifact);
        library.push(previous.row);
        payloads.set(previous.row.catalogKey, previous.placement);
        nextEntries.set(type, previous);
        continue;
      }
      const catalogKey = generation === 0 ? descriptor.key : `${descriptor.key}:${generation}`;
      if (!isRecord(cached.document))
        throw new Error(`Cube '${descriptor.displayName}' has no portable document.`);
      const metadata: UnknownRecord = {
        ...cached.cube,
        cube_version: descriptor.version,
        default_alias: descriptor.displayName,
        target_model: descriptor.targetModel,
      };
      const artifact: CubeStackArtifact = {
        catalogKey,
        cubeId: descriptor.cubeId,
        version: descriptor.version,
        alias: descriptor.displayName,
        document: cached.document,
        size: construction.previewSize(cached, descriptor.inputs.length > 0, {
          instanceId: 'cart-frame-preview',
        }),
      };
      const placement = { descriptor, payload: cached };
      const row: CubeStackLibraryRow = {
        ...cubeStackCartRow(catalogKey, metadata, descriptor.displayName, false),
        catalogKey,
        models: descriptor.supportedModels.length
          ? descriptor.supportedModels
          : [descriptor.targetModel || 'Other models'],
        role:
          descriptor.inputs.length === 0
            ? descriptor.outputs.length
              ? 'Start cubes'
              : 'Other cubes'
            : descriptor.outputs.length
              ? 'Middle cubes'
              : 'End cubes',
        searchText: [
          descriptor.displayName,
          descriptor.cubeId,
          descriptor.description,
          descriptor.targetModel,
          ...descriptor.supportedModels,
          ...descriptor.searchTerms,
        ]
          .join(' ')
          .toLocaleLowerCase(),
      };
      artifacts.push(artifact);
      library.push(row);
      payloads.set(catalogKey, placement);
      nextEntries.set(type, { fingerprint, artifact, row, placement });
    }
    this.#entries = nextEntries;
    return { artifacts, library, payloads };
  }
}
