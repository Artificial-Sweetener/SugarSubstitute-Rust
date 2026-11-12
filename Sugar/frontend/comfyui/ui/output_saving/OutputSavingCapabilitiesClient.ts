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
/** Validate the backend's explicit saver qualification catalog at the browser boundary. */
import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import { isRecord } from '../types/common.js';

export interface SavingNodeCapability {
  readonly classType: string;
  readonly managed: boolean;
  readonly kind: 'save' | 'preview' | null;
}

/** Fetch qualification facts without guessing whether arbitrary output nodes perform saving. */
export class OutputSavingCapabilitiesClient {
  /** Bind the existing host-addressed transport. */
  constructor(private readonly api: Pick<CubeLibraryApi, 'fetchJson'>) {}

  /** Bound catalog retrieval and narrow every dynamic class declaration. */
  async load(): Promise<ReadonlyMap<string, SavingNodeCapability>> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20000);
    try {
      const { response, data } = await this.api.fetchJson(
        '/sugarcubes/output-saving/capabilities',
        { signal: controller.signal },
      );
      if (
        response.ok === false ||
        data.success !== true ||
        !isRecord(data.data) ||
        !Array.isArray(data.data.nodes)
      )
        throw new Error('Output saving capabilities are unavailable.');
      const result = new Map<string, SavingNodeCapability>();
      for (const node of data.data.nodes) {
        if (
          !isRecord(node) ||
          typeof node.class_type !== 'string' ||
          !node.class_type ||
          typeof node.managed !== 'boolean' ||
          (node.kind !== 'save' && node.kind !== 'preview' && node.kind !== null) ||
          (node.managed && node.kind === null) ||
          result.has(node.class_type)
        )
          throw new Error('Invalid output saving capabilities.');
        result.set(
          node.class_type,
          Object.freeze({ classType: node.class_type, managed: node.managed, kind: node.kind }),
        );
      }
      return result;
    } finally {
      clearTimeout(timer);
    }
  }
}
