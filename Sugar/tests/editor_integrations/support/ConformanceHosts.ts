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
import { CanvasMaskServer } from '../../../integrations/shared/CanvasMaskServer.js';
import type { InMemoryHostHandler } from '../../../integrations/shared/conformance/InMemoryHostChannel.js';
import { isMemoryUsageMessage } from '../../../integrations/shared/MemoryValidation.generated.js';
import { isUpdatesClientMessage } from '../../../integrations/shared/UpdatesValidation.generated.js';
import { isReleaseNotesClientMessage } from '../../../integrations/shared/ReleaseNotesValidation.generated.js';
import { isSessionNoticeClientMessage } from '../../../integrations/shared/SessionNoticeValidation.generated.js';

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

/** A mask owner with no document open, as an editor panel answers before one is opened. */
export function masksWithoutDocument(send: (message: unknown) => void): InMemoryHostHandler {
  return new CanvasMaskServer(conformanceSession, {
    changes: null,
    execute: (command) =>
      Promise.resolve(
        command.type === 'context'
          ? { kind: 'context', supported: true, document: null, observesChanges: false }
          : { kind: 'failure', status: 'unavailable', message: 'No document is open.' },
      ),
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

/** A host that names its product and running version and has no notes published. */
export function releaseNotesHost(
  send: (message: unknown) => void,
  product = 'Conformance Host',
): InMemoryHostHandler {
  const envelope = {
    protocol: 'sugarcubes.release-notes',
    version: 1,
    session: conformanceSession,
  };
  return {
    receive: (value) => {
      if (!isReleaseNotesClientMessage(value)) return false;
      if (value.type === 'watch')
        send({ ...envelope, type: 'state', revision: 1, product, currentVersion: '1.0.0' });
      else send({ ...envelope, type: 'failed', request: value.request, reason: 'not-published' });
      return true;
    },
  };
}

/** A host that owes one notice of unsaved work until the page says it was shown. */
export function sessionNoticeHost(
  send: (message: unknown) => void,
  lost: { readonly workspace: boolean; readonly inputs: boolean } = {
    workspace: true,
    inputs: false,
  },
): InMemoryHostHandler {
  const envelope = {
    protocol: 'sugarcubes.session-notice',
    version: 1,
    session: conformanceSession,
  };
  let owed = true;
  return {
    receive: (value) => {
      if (!isSessionNoticeClientMessage(value)) return false;
      if (value.type === 'shown') owed = false;
      else
        send(
          owed
            ? { ...envelope, type: 'not-saved', notice: 1, ...lost }
            : { ...envelope, type: 'none' },
        );
      return true;
    },
  };
}
