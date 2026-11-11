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
/** One finite latest-wins clipboard lane validates external results and settles retirement immediately. */
import { LatestRequest } from '../requests/latest.js';
import type {
  ClipboardGateway,
  ClipboardReadOutcome,
  ClipboardReadResult,
  ClipboardTransfer,
  ClipboardWriteOutcome,
  ClipboardWriteResult,
} from './contracts.js';

/** Validate native clipboard envelopes without including source or arbitrary error payloads in rejection messages. */
function readResult(value: unknown): ClipboardReadResult {
  if (typeof value !== 'object' || value === null || Array.isArray(value) || !('status' in value))
    throw new TypeError('Invalid clipboard read result');
  if (value.status === 'read' && 'value' in value) {
    const transfer = value.value;
    if (
      typeof transfer === 'object' &&
      transfer !== null &&
      !Array.isArray(transfer) &&
      'kind' in transfer &&
      (transfer.kind === 'exact' || transfer.kind === 'plain') &&
      'text' in transfer &&
      typeof transfer.text === 'string'
    )
      return {
        status: 'read',
        value: { kind: transfer.kind, text: transfer.text },
      };
  } else if (
    value.status === 'empty' ||
    value.status === 'denied' ||
    value.status === 'unavailable'
  )
    return { status: value.status };
  throw new TypeError('Invalid clipboard read result');
}

/** Validate expected write availability separately from request lifetime and source mutation. */
function writeResult(value: unknown): ClipboardWriteResult {
  if (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    'status' in value &&
    (value.status === 'written' || value.status === 'denied' || value.status === 'unavailable')
  )
    return { status: value.status };
  throw new TypeError('Invalid clipboard write result');
}

/** Read and write share a single external request lifetime; editor selection/source remain outside this owner. */
export class ClipboardSession {
  private readonly lane = new LatestRequest();
  private readonly gateway: ClipboardGateway;

  /** The native adapter owns actual browser capabilities; this owner validates its publication. */
  constructor(gateway: ClipboardGateway) {
    this.gateway = gateway;
  }

  /** Supersede any earlier clipboard work and publish only a current validated transfer. */
  async read(nativeTransfer?: ClipboardTransfer): Promise<ClipboardReadOutcome> {
    const outcome = await this.lane.run(async (signal) =>
      this.gateway.readable
        ? readResult(await this.gateway.read(signal, nativeTransfer))
        : { status: 'unavailable' as const },
    );
    return outcome.status === 'ready' ? outcome.value : outcome;
  }

  /** Preserve the deliberate captured text while preventing late completion from authorizing a source edit. */
  async write(text: string): Promise<ClipboardWriteOutcome> {
    const outcome = await this.lane.run(async (signal) =>
      this.gateway.writable
        ? writeResult(await this.gateway.write(text, signal))
        : { status: 'unavailable' as const },
    );
    return outcome.status === 'ready' ? outcome.value : outcome;
  }

  /** Settle current work and release publication authority on source, selection or focus retirement. */
  cancel(): void {
    this.lane.cancel();
  }

  /** Settle ignored-abort work before native editor teardown. */
  dispose(): void {
    this.lane.dispose();
  }
}
