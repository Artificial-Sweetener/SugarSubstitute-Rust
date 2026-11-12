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
/** Compose in-memory hosts from the shared owners for the conformance kit's own proofs. */
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import type { InMemoryHostHandler } from '../../../integrations/shared/conformance/InMemoryHostChannel.js';
import { isMemoryUsageMessage } from '../../../integrations/shared/MemoryValidation.generated.js';
import { isUpdatesClientMessage } from '../../../integrations/shared/UpdatesValidation.generated.js';

export const conformanceSession = 'panel-0123456789abcdef';

/** A canvas server with no output surface, as the editor panels configure theirs. */
export function canvasWithoutSurface(send: (message: unknown) => void): InMemoryHostHandler {
  return new CanvasServer(conformanceSession, {
    context: () => Promise.reject(new Error('The conformance host has no document.')),
    insert: () => Promise.reject(new Error('The conformance host inserts nothing.')),
    send,
    report: (error) => {
      throw error;
    },
  });
}

/** A memory budget owner answering every usage report with one allowance. */
export function memoryHost(
  send: (message: unknown) => void,
  allowance: Record<string, unknown> = {},
): InMemoryHostHandler {
  let generation = 0;
  return {
    receive: (value) => {
      if (!isMemoryUsageMessage(value)) return false;
      send({
        protocol: 'sugarcubes.memory',
        version: 1,
        session: conformanceSession,
        type: 'allowance',
        generation: ++generation,
        level: 'normal',
        allowanceBytes: 268435456,
        ceilingBytes: 1073741824,
        totalBytes: 68719476736,
        preference: { mode: 'share', percent: 25 },
        ...allowance,
      });
      return true;
    },
  };
}

/** An updater that reports up to date and accepts every check. */
export function updatesHost(send: (message: unknown) => void): InMemoryHostHandler {
  const envelope = { protocol: 'sugarcubes.updates', version: 1, session: conformanceSession };
  return {
    receive: (value) => {
      if (!isUpdatesClientMessage(value)) return false;
      if (value.type === 'watch')
        send({
          ...envelope,
          type: 'state',
          revision: 1,
          update: {
            currentVersion: '1.0.0',
            channel: 'stable',
            status: { state: 'up-to-date' },
            previousVersionRunning: false,
            workflowRunning: false,
            scheduledRestart: 'none',
          },
        });
      else send({ ...envelope, type: 'accepted', request: value.request });
      return true;
    },
  };
}
