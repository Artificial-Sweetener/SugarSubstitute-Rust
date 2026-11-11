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
/** Expose validated download progress independently of modal and native-field lifetimes. */
export interface ModelTransferProgress {
  readonly message: string;
  readonly percentage: number | null;
}
/** Retain the accepted model's prepared preview independently of the current field selection. */
export interface ModelTransferModel {
  readonly name: string;
  readonly previewUrl: string | null;
}
export interface ModelTransferState extends ModelTransferProgress {
  readonly model: ModelTransferModel;
}
export type ModelTransferObserver = (progress: ModelTransferProgress) => void;
export type ModelTransferOperation = (
  progress: ModelTransferObserver,
  signal: AbortSignal,
) => Promise<string>;

/** Commit through a live field receiver after verified publication and host choice refresh. */
export interface ModelSelectionTarget {
  current(): string;
  select(filename: string): void;
  download(
    operation: ModelTransferOperation,
    model: ModelTransferModel,
    signal?: AbortSignal,
  ): Promise<string>;
  useDownload(operation: ModelTransferOperation, model: ModelTransferModel): Promise<void>;
}
