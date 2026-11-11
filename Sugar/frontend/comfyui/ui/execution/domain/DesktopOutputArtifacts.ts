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
/** Keep verified artifact resolution separate from workflow membership and durable execution identity. */
import type {
  SessionBinding,
  SessionOutput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type { DesktopSessionCancellation } from '../../workflow/domain/DesktopSession.js';
import type { OutputFileReference } from './WorkflowOutput.js';

export type DesktopArtifactReceipt = Pick<
  SessionOutput,
  'file' | 'digest' | 'bytes' | 'width' | 'height' | 'media' | 'mime' | 'metadataEmbedded'
> & { readonly animated: boolean };
export type DesktopArtifactResult =
  | { readonly available: true; readonly receipt: DesktopArtifactReceipt; readonly url?: string }
  | { readonly available: false; readonly code: string; readonly message: string };

/** Accept only exact file receipts or validated immutable recorded outputs, never history guesses. */
export interface DesktopOutputArtifacts {
  receipt(
    file: OutputFileReference,
    cancellation: DesktopSessionCancellation,
  ): Promise<DesktopArtifactResult>;
  resolve(
    output: SessionOutput,
    binding: SessionBinding,
    cancellation: DesktopSessionCancellation,
  ): Promise<DesktopArtifactResult>;
}
