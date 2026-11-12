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
/** Supply deterministic native channel replies while exercising the actual shared mask protocol. */
import type {
  NativeCommand,
  NativeResult,
} from '../../../integrations/gimp/src/contracts/NativeProtocol.generated.js';
import { GimpCanvasMasks } from '../../../integrations/gimp/src/infrastructure/GimpCanvasMasks.js';
import { CanvasMaskClient } from '../../../integrations/shared/CanvasMaskClient.js';
import { CanvasMaskServer } from '../../../integrations/shared/CanvasMaskServer.js';
import { encodeCanvasBytes } from '../../../integrations/shared/CanvasByteEncoding.js';
import { NativeCanvasFixture } from './native_canvas.js';

/** Retain only external channel data and frozen SDK values, without reproducing shared mask validation. */
export class NativeMaskFixture extends NativeCanvasFixture {
  readonly channels = new Map<number, string>();
  readonly frozen = new Map<string, ReadonlyMap<number, string>>();
  selected: number | null = null;
  #next = 50;

  /** Implement the private channel boundary while the production adapter owns public command interpretation. */
  override async execute(command: NativeCommand): Promise<NativeResult> {
    if (command.type === 'channel.create') {
      this.commands.push(command);
      const tattoo = this.#next++;
      this.channels.set(
        tattoo,
        command.data ??
          encodeCanvasBytes(
            new Uint8Array(this.document.width * this.document.height).fill(
              command.source === 'selection' ? 127 : 0,
            ),
          ),
      );
      return { kind: 'channel-created', tattoo, committed: true };
    }
    if (command.type === 'channel.select' || command.type === 'channel.release') {
      this.commands.push(command);
      if (command.type === 'channel.select' && !this.channels.has(command.tattoo))
        return { kind: 'failure', status: 'stale', message: 'The native channel has closed.' };
      this.selected = command.type === 'channel.select' ? command.tattoo : null;
      return { kind: 'channel-selected', committed: true };
    }
    if (command.type === 'snapshot.channel') {
      this.commands.push(command);
      const data = this.frozen.get(command.token)?.get(command.tattoo);
      if (!data)
        return {
          kind: 'failure',
          status: 'stale',
          message: 'The frozen native channel is unavailable.',
        };
      return {
        kind: 'pixels',
        width: command.width,
        height: command.height,
        format: 'coverage8',
        data,
      };
    }
    const result = await super.execute(command);
    if (result.kind === 'snapshot') this.frozen.set(result.token, new Map(this.channels));
    if (command.type === 'snapshot.release' && result.kind === 'released')
      this.frozen.delete(command.token);
    return result;
  }
}

/** Bind production validation, request correlation, mask translation and native capture ownership. */
export function nativeMaskConnection(native: NativeMaskFixture) {
  const errors: unknown[] = [];
  const host = new GimpCanvasMasks(native, (error) => errors.push(error));
  const session = '00000000-0000-4000-8000-000000000001';
  const server = new CanvasMaskServer(session, {
    changes: host.changes,
    execute: (command) => host.execute(command),
    send: (message) => {
      client.receive(message);
    },
    report: (error) => errors.push(error),
  });
  const client = new CanvasMaskClient(
    session,
    {
      send: (message) => {
        server.receive(message);
      },
      later: () => () => {},
      report: (error) => errors.push(error),
    },
    () => 'gimp',
  );
  return {
    client,
    errors,
    dispose: () => {
      client.dispose();
      server.dispose();
      host.dispose();
    },
  };
}
