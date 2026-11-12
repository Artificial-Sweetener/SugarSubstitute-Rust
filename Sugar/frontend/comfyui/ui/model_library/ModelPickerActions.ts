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
/** Declare model-library actions without coupling Vue presentation to HTTP transport. */
import type { CivitaiCredentialPort } from './ModelDiscovery.js';
import type { ModelTransferObserver } from './ModelTransfer.js';
import type { ModelRelease } from './ModelReleases.js';
export type ModelPickerFeedback = (severity: 'info' | 'error', message: string) => void;
export interface ModelPickerActions {
  readonly credentials: CivitaiCredentialPort;
  classifyContent(modelId: string, nsfw: boolean | null, signal: AbortSignal): Promise<void>;
  refreshMetadata(
    modelId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
  ): Promise<void>;
  replaceThumbnail(modelId: string, image: File, signal: AbortSignal): Promise<void>;
  releases(modelId: string, signal: AbortSignal): Promise<readonly ModelRelease[]>;
  acquire(
    modelId: string,
    versionId: string,
    fileId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
    transfer?: ModelTransferObserver,
  ): Promise<string>;
}
