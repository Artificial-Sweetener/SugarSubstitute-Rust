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
/** Define reviewed discovery selections without client-owned download URLs or destinations. */

import type { ModelTransferObserver } from './ModelTransfer.js';
export interface ModelDiscoveryContext {
  readonly artifactKind: string;
  readonly familyId: string | null;
}
export interface ModelDiscoveryOffer {
  readonly providerId: string;
  readonly providerName: string;
  readonly modelId: string;
  readonly versionId: string;
  readonly modelPageUrl: string;
  readonly fileName: string;
  readonly sizeBytes: number;
  readonly access: 'public' | 'api_key_required';
  readonly previewUrl: string | null;
}
export interface ModelDiscoverySuggestion {
  readonly identity: string;
  readonly modelName: string;
  readonly versionName: string;
  readonly creator: string | null;
  readonly offers: readonly ModelDiscoveryOffer[];
}
export interface ModelDiscoveryPlan extends ModelDiscoveryContext {
  readonly planId: string;
  readonly publicOnly: boolean;
  readonly suggestions: readonly ModelDiscoverySuggestion[];
  readonly browseUrls: readonly { readonly providerId: string; readonly url: string }[];
}
export interface ModelDiscoveryPort {
  context(
    kind: string,
    targetModel: string,
    signal: AbortSignal,
  ): Promise<ModelDiscoveryContext | null>;
  discover(
    kind: string,
    targetModel: string,
    publicOnly: boolean,
    progress: (message: string) => void,
    signal: AbortSignal,
  ): Promise<ModelDiscoveryPlan>;
  acquire(
    planId: string,
    identity: string,
    providerId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
    transfer?: ModelTransferObserver,
  ): Promise<string>;
  release(planId: string): Promise<void>;
}
export interface CivitaiCredentialStatus {
  readonly configured: boolean;
  readonly storageAvailable: boolean;
  readonly storageReason: string | null;
}
export interface CivitaiCredentialPort {
  status(signal: AbortSignal): Promise<CivitaiCredentialStatus>;
  save(key: string, signal: AbortSignal): Promise<CivitaiCredentialStatus>;
}
