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
/** Own workflow-independent Cube runtime caches for the page lifetime. */
import { CubeIconImageStore } from '../core/CubeIconImageStore.js';
import type { MemoryBudgetRegistry } from '../core/memory/MemoryBudgetRegistry.js';
import { CubePreviewImageStore } from '../surface/CubeCanvasPreviewImageCache.js';
import type { CubeCanvasImageStores } from '../surface/CubeCanvasImageStores.js';
import { CubeVersionAvailabilityService } from './version/CubeVersionAvailabilityService.js';
import { CubeVersionRepository, type CubeVersionApi } from './version/CubeVersionRepository.js';

/**
 * Keep caches keyed by Cube identity, version, content or URL across workflow switches.
 *
 * `ComfyCubeRuntime` is rebuilt for every configured graph and keeps only workflow-bound state;
 * it borrows these owners instead of discarding their decoded images and fetched artifacts.
 */
export class ComfyCubeRuntimeCaches {
  readonly images: CubeCanvasImageStores;
  readonly versionRepository: CubeVersionRepository;
  readonly versionAvailability: CubeVersionAvailabilityService;

  /** Register every cache with the page memory allowance. */
  constructor(memory: MemoryBudgetRegistry, api: CubeVersionApi, logger: Pick<Console, 'warn'>) {
    this.images = {
      icons: new CubeIconImageStore(memory),
      previews: new CubePreviewImageStore(memory),
    };
    this.versionRepository = new CubeVersionRepository(api, memory, logger);
    this.versionAvailability = new CubeVersionAvailabilityService(this.versionRepository);
  }

  /** Retire fetched version history and artifacts after a library mutation. */
  invalidateLibrary(): void {
    this.versionAvailability.invalidate();
    this.versionRepository.invalidate();
  }
}
