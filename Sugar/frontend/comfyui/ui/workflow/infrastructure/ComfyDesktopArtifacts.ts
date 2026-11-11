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
/** Adapt exact backend artifact receipts and native retained-media facts to the desktop session use case. */
import type {
  SessionBinding,
  SessionOutput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type {
  DesktopOutputArtifacts,
  DesktopArtifactResult,
} from '../../execution/domain/DesktopOutputArtifacts.js';
import type { OutputFileReference } from '../../execution/domain/WorkflowOutput.js';
import type { OutputArtifactClient } from '../../output_saving/OutputArtifactClient.js';
import type { DesktopSessionCancellation } from '../domain/DesktopSession.js';
import { DesktopSessionRefusal } from '../domain/DesktopSession.js';
import type { BrowserDesktopAssets } from './BrowserDesktopAssets.js';

/** Prefer the verified backend original so its existing reveal, rendition and Photoshop capabilities remain available. */
export class ComfyDesktopArtifacts implements DesktopOutputArtifacts {
  constructor(
    private readonly backend: OutputArtifactClient,
    private readonly retained: BrowserDesktopAssets,
  ) {}

  async receipt(
    file: OutputFileReference,
    cancellation: DesktopSessionCancellation,
  ): Promise<DesktopArtifactResult> {
    return this.#request(cancellation, (signal) => this.backend.receipt(file, signal));
  }

  async resolve(
    output: SessionOutput,
    binding: SessionBinding,
    cancellation: DesktopSessionCancellation,
  ): Promise<DesktopArtifactResult> {
    const backend = await this.#request(cancellation, (signal) =>
      this.backend.resolve(output, signal),
    );
    if (backend.available || output.assetDigest === null || cancellation.cancelled())
      return backend;
    try {
      const presentation = await this.retained.presentation(output, binding, cancellation);
      return {
        available: true,
        receipt: {
          file: output.file,
          digest: output.digest,
          bytes: output.bytes,
          width: output.width,
          height: output.height,
          media: output.media,
          mime: output.mime,
          metadataEmbedded: output.metadataEmbedded,
          animated: presentation.animated,
        },
        url: presentation.url,
      };
    } catch (error: unknown) {
      if (error instanceof DesktopSessionRefusal) {
        if (error.code === 'cancelled') throw error;
        return { available: false, code: error.code, message: error.message };
      }
      throw error;
    }
  }

  /** Tie every backend request to the owning protocol cancellation rather than a detached request lifetime. */
  async #request(
    cancellation: DesktopSessionCancellation,
    request: (signal: AbortSignal) => Promise<DesktopArtifactResult>,
  ): Promise<DesktopArtifactResult> {
    const controller = new AbortController();
    const stop = cancellation.subscribe(() => controller.abort());
    try {
      return await request(controller.signal);
    } finally {
      stop();
    }
  }
}
