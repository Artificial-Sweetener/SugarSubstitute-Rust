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
/** Supply a typed private-native boundary while retaining the real public canvas client and server. */
import type { GimpNativePort } from '../../../integrations/gimp/src/application/NativeGimpSession.js';
import type {
  NativeCommand,
  NativeDocument,
  NativeResult,
  NativeLayer,
} from '../../../integrations/gimp/src/contracts/NativeProtocol.generated.js';
import { GimpCanvasBridge } from '../../../integrations/gimp/src/infrastructure/GimpCanvasBridge.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { encodeCanvasBytes } from '../../../integrations/shared/CanvasByteEncoding.js';

/** Carry a real three-by-two PNG created independently from the shared PNG admission code. */
export const nativeFixturePng = {
  format: 'png',
  encoding: 'base64',
  data: 'iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAYAAACddGYaAAAAFElEQVR4nGOUO/HhPwMUMMEYIBoAQXYC2XHUFOoAAAAASUVORK5CYII=',
} as const;

/** Model only external SDK replies; source interpretation remains in production adapters. */
export class NativeCanvasFixture implements GimpNativePort {
  document: NativeDocument = {
    imageId: 7,
    revision: 'geometry',
    width: 8,
    height: 6,
    name: '<native image>',
  };
  rgb = true;
  layers: readonly NativeLayer[] = [];
  primaryLayer: number | null = null;
  readonly commands: NativeCommand[] = [];
  readonly snapshots = new Set<string>();
  releaseFailures = 0;
  staleSnapshot = false;
  #serial = 0;

  /** Return deterministic primitive values without constructing a second canvas protocol server. */
  async execute(command: NativeCommand): Promise<NativeResult> {
    this.commands.push(command);
    if (command.type === 'context')
      return {
        kind: 'context',
        hostId: 'fixture-host',
        document: { ...this.document },
        layers: this.layers,
        primaryLayer: this.primaryLayer,
        rgb: this.rgb,
      };
    if (command.type === 'snapshot.begin') {
      const token = `snapshot-${String(++this.#serial)}`;
      this.snapshots.add(token);
      return {
        kind: 'snapshot',
        token,
        document: { ...this.document, width: this.document.width + (this.staleSnapshot ? 1 : 0) },
        layers: this.layers,
        primaryLayer: this.primaryLayer,
      };
    }
    if (command.type === 'snapshot.release') {
      if (this.releaseFailures > 0) {
        this.releaseFailures--;
        return { kind: 'failure', status: 'error', message: 'Native release failed.' };
      }
      this.snapshots.delete(command.token);
      return { kind: 'released' };
    }
    if (command.type === 'image.insert')
      return {
        kind: 'inserted',
        committed: true,
        tattoo: 31,
        left: command.left,
        top: command.top,
        width: command.width,
        height: command.height,
      };
    if (command.type === 'geometry.context')
      return {
        kind: 'geometry',
        document: this.document,
        selection: { left: 1, top: 2, width: 3, height: 2 },
      };
    if (command.type === 'image.resize') {
      this.document = {
        ...this.document,
        width: command.width,
        height: command.height,
        revision: `resized-${String(++this.#serial)}`,
      };
      return { kind: 'resized', document: this.document, committed: true };
    }
    if (
      command.type === 'channel.create' ||
      command.type === 'channel.select' ||
      command.type === 'channel.release'
    )
      return {
        kind: 'failure',
        status: 'unsupported',
        message: 'This fixture models read-only channels.',
      };
    if (!this.snapshots.has(command.token))
      return { kind: 'failure', status: 'stale', message: 'Native snapshot closed.' };
    const selection = command.type === 'snapshot.selection';
    return {
      kind: 'pixels',
      width: command.width,
      height: command.height,
      format: selection ? 'coverage8' : 'rgba8',
      data: encodeCanvasBytes(
        new Uint8Array(command.width * command.height * (selection ? 1 : 4)).fill(
          selection ? 255 : 7,
        ),
      ),
    };
  }
}

/** Exercise actual shared wire validation, request correlation and the native GIMP adapter together. */
export function nativeCanvasConnection(native: GimpNativePort) {
  const errors: unknown[] = [];
  const session = '00000000-0000-4000-8000-000000000001';
  const bridge = new GimpCanvasBridge(
    session,
    native,
    (message) => {
      client.receive(message);
    },
    (error) => errors.push(error),
  );
  const client = new CanvasClient(session, {
    send: (message) => {
      bridge.receive(message);
    },
    later: () => () => {},
    report: (error) => errors.push(error),
  });
  return {
    client,
    errors,
    dispose: () => {
      client.dispose();
      bridge.dispose();
    },
  };
}
