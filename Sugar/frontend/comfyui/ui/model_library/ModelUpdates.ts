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
/** Validate the shared server-owned update feed before rendering notification actions. */
import { isRecord } from '../types/common.js';

export interface ModelUpdateOffer {
  readonly modelId: string;
  readonly sha256: string;
  readonly modelName: string;
  readonly versionId: string;
}
export interface ModelUpdateSnapshot {
  readonly enabled: boolean;
  readonly proposals: readonly ModelUpdateOffer[];
}
export interface ModelUpdatePort {
  read(): ModelUpdateSnapshot;
  subscribe(listener: (snapshot: ModelUpdateSnapshot) => void): () => void;
  load(): Promise<void>;
  check(): Promise<void>;
  checkSelection(modelId: string): Promise<void>;
  setEnabled(enabled: boolean): Promise<void>;
  dismiss(modelId: string): Promise<void>;
  disableModel(modelId: string): Promise<void>;
}

/** Reject unknown consent or malformed exact-model update identities at the browser boundary. */
export function decodeModelUpdates(value: unknown): ModelUpdateSnapshot {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    typeof value.enabled !== 'boolean' ||
    !Array.isArray(value.proposals)
  )
    throw new Error('Invalid model update response.');
  return {
    enabled: value.enabled,
    proposals: value.proposals.map((item: unknown) => {
      if (
        !isRecord(item) ||
        typeof item.modelId !== 'string' ||
        !/^[a-f0-9]{64}$/.test(item.modelId) ||
        typeof item.sha256 !== 'string' ||
        !/^[a-f0-9]{64}$/.test(item.sha256) ||
        typeof item.modelName !== 'string' ||
        typeof item.versionId !== 'string' ||
        !/^[1-9][0-9]{0,15}$/.test(item.versionId)
      )
        throw new Error('Invalid model update offer.');
      return {
        modelId: item.modelId,
        sha256: item.sha256,
        modelName: item.modelName,
        versionId: item.versionId,
      };
    }),
  };
}
