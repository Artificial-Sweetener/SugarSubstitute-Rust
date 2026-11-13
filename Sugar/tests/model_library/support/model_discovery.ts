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
/** Supply reviewed, inert discovery offers and independently observed host commands. */
import { jest } from '@jest/globals';
import type {
  ModelDiscoveryPlan,
  ModelDiscoveryPort,
  CivitaiCredentialPort,
} from '../../../frontend/comfyui/ui/model_library/ModelDiscovery.js';

/** Keep alternate public and credential-protected offers attached to one content identity. */
export function discoveryPlan(
  publicOnly = false,
  artifactKind = 'upscale_models',
): ModelDiscoveryPlan {
  return {
    artifactKind,
    familyId: null,
    planId: 'a'.repeat(32),
    publicOnly,
    browseUrls: [{ providerId: 'openmodeldb', url: 'https://openmodeldb.info/' }],
    suggestions: [
      {
        identity: artifactKind + ':' + 'b'.repeat(64),
        modelName: '<b>Reviewed portrait</b>',
        versionName: 'Original',
        creator: null,
        offers: [
          {
            providerId: 'openmodeldb',
            providerName: 'OpenModelDB',
            modelId: 'portrait',
            versionId: 'original',
            modelPageUrl: 'https://openmodeldb.info/models/portrait',
            fileName: 'portrait.pth',
            sizeBytes: 32,
            access: 'public',
            previewUrl: null,
          },
          {
            providerId: 'civitai',
            providerName: 'CivitAI',
            modelId: '100',
            versionId: '101',
            modelPageUrl: 'https://civitai.com/models/100',
            fileName: 'portrait.pth',
            sizeBytes: 32,
            access: 'api_key_required',
            previewUrl: null,
          },
        ],
      },
    ],
  };
}

/** Expose deterministic commands without contacting providers or saving real credentials. */
export function discoveryPorts() {
  const context = jest.fn<ModelDiscoveryPort['context']>(async (artifactKind) => ({
    artifactKind,
    familyId: null,
  }));
  const discover = jest.fn<ModelDiscoveryPort['discover']>(async (kind, _target, publicOnly) =>
    discoveryPlan(publicOnly, kind),
  );
  const acquire = jest.fn<ModelDiscoveryPort['acquire']>(async () => 'portrait.pth');
  const release = jest.fn<ModelDiscoveryPort['release']>(async () => undefined);
  const status = jest.fn<CivitaiCredentialPort['status']>(async () => ({
    configured: false,
    storageAvailable: true,
    storageReason: null,
  }));
  const save = jest.fn<CivitaiCredentialPort['save']>(async () => ({
    configured: true,
    storageAvailable: true,
    storageReason: null,
  }));
  return { discovery: { context, discover, acquire, release }, credentials: { status, save } };
}
