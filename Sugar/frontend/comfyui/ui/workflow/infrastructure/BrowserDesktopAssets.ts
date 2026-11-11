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
/** Read bounded metadata for exact immutable retained finals through the native same-origin capability. */
import type {
  SessionBinding,
  SessionOutput,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type {
  DesktopSessionCancellation,
  DesktopSessionTransport,
} from '../domain/DesktopSession.js';
import { DesktopSessionRefusal } from '../domain/DesktopSession.js';

export interface DesktopAssetPresentation {
  readonly url: string;
  readonly animated: boolean;
}

/** Keep retained bytes and digest verification with the native owner; expose only current-page presentation. */
export class BrowserDesktopAssets {
  /** Inject current-origin HTTP and page timers without creating another asset store or resolver. */
  constructor(
    private readonly transport: DesktopSessionTransport,
    private readonly request: typeof globalThis.fetch,
    private readonly clock: Pick<Window, 'setTimeout' | 'clearTimeout'>,
  ) {}

  /** Probe verified current asset facts; WebP animation is a byte-derived fact, never a filename heuristic. */
  async presentation(
    output: SessionOutput,
    binding: SessionBinding,
    cancellation: DesktopSessionCancellation,
  ): Promise<DesktopAssetPresentation> {
    if (
      output.assetDigest === null ||
      output.assetDigest !== output.digest ||
      binding.entryId !== output.entryId
    )
      throw new TypeError('Retained output does not belong to this admitted desktop occurrence.');
    const url = this.transport.assetUrl?.(output.id, binding);
    if (!url)
      throw new DesktopSessionRefusal(
        'unavailable',
        'Retained output presentation capability is unavailable.',
      );
    const controller = new AbortController();
    const stop = cancellation.subscribe(() => controller.abort());
    const timer = this.clock.setTimeout(() => controller.abort(), 15000);
    try {
      const response = await this.request(url, { method: 'HEAD', signal: controller.signal });
      if (cancellation.cancelled())
        throw new DesktopSessionRefusal(
          'cancelled',
          'Retained output metadata request was cancelled.',
        );
      if (!response.ok)
        throw new DesktopSessionRefusal(
          'unavailable',
          `Retained output metadata is unavailable (${response.status}).`,
        );
      const bytes = response.headers.get('content-length');
      const mime = response.headers.get('content-type')?.split(';')[0]?.trim().toLowerCase();
      const animated = response.headers.get('x-sugarsubstitute-animated');
      if (
        bytes === null ||
        !/^[0-9]+$/.test(bytes) ||
        Number(bytes) !== output.bytes ||
        mime !== output.mime ||
        (animated !== '0' && animated !== '1')
      )
        throw new DesktopSessionRefusal(
          'identity-mismatch',
          'Retained output metadata does not match the exact recorded artifact.',
        );
      return Object.freeze({ url, animated: animated === '1' });
    } finally {
      this.clock.clearTimeout(timer);
      stop();
    }
  }
}
