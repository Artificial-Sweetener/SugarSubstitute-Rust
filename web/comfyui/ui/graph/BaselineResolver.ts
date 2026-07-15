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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/BaselineResolver.js`.
 */

/**
 * Coordinate baseline resolver behavior for the SugarCubes UI.
 */
export interface BaselineLookup {
  getDefinitionHash(key: string | undefined): string | null | undefined;
  getDefinitionStatus(key: string | undefined): string | null | undefined;
  getLocalBaselineHash(instanceId: string | undefined): string | null | undefined;
}

export interface BaselineResolverOptions {
  baselineStore?: BaselineLookup | null;
}

export interface BaselineRequest {
  cubeId?: string;
  definitionKey?: string;
  instanceId?: string;
  missingSymbols?: boolean;
}

export interface BaselineResolution {
  baselineHash: string | null;
  baselineSource: 'local' | 'definition';
  useDefinition: boolean;
}

/** Resolve the authoritative dirty-state baseline for one cube instance. */
export class BaselineResolver {
  private readonly baselineStore: BaselineLookup | null;

  constructor({ baselineStore }: BaselineResolverOptions = {}) {
    this.baselineStore = baselineStore || null;
  }

  resolve({
    cubeId,
    definitionKey,
    instanceId,
    missingSymbols,
  }: BaselineRequest = {}): BaselineResolution {
    const key = definitionKey || cubeId;
    const definitionHash = this.baselineStore?.getDefinitionHash(key) || null;
    const definitionStatus = this.baselineStore?.getDefinitionStatus(key) || null;
    const definitionReady = definitionStatus == null || definitionStatus === 'ready';
    const canUseDefinition = Boolean(definitionHash && definitionReady && !missingSymbols);

    const localBaselineHash = this.baselineStore?.getLocalBaselineHash(instanceId) || null;
    if (localBaselineHash) {
      return { baselineHash: localBaselineHash, baselineSource: 'local', useDefinition: false };
    }

    if (canUseDefinition) {
      return { baselineHash: definitionHash, baselineSource: 'definition', useDefinition: true };
    }

    return { baselineHash: null, baselineSource: 'local', useDefinition: false };
  }
}
