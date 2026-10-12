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
/** Bind an isolated overlay root to one revisioned surface port. */

import { createApp, h, shallowRef } from 'vue';
import { sameCanvasAttachment } from '../../domain/CanvasIdentity.js';
import type {
  CanvasCommand,
  CanvasCommandRequest,
  CanvasViewPort,
} from '../../domain/CanvasViewPort.js';
import { CanvasOverlay } from './CanvasOverlay.js';

export interface MountedCanvasOverlay {
  /** Release this root's subscription and controls without releasing its surface. */
  dispose(): void;
}

/** Subscribe and release only presentation; surface resource lifetime stays with its owner. */
export function mountCanvasOverlay(
  element: HTMLElement,
  port: CanvasViewPort,
  reportFailure: (error: unknown, request: CanvasCommandRequest | null) => void,
): MountedCanvasOverlay {
  const initial = port.read();
  const state = shallowRef(initial);
  const announcement = shallowRef('');
  let active = true;
  let sequence = 0;
  /** Dispatch a capability-gated intent with its exact originating attachment and revision. */
  async function send(command: CanvasCommand): Promise<void> {
    if (!active || !port.capabilities.commands.includes(command.type)) return;
    const requestId = ++sequence;
    const request: CanvasCommandRequest = {
      schema: 1,
      address: initial.address,
      requestId,
      expectedRevision: state.value.revision,
      command,
    };
    try {
      const outcome = await port.dispatch(request);
      if (!active || requestId !== sequence) return;
      if (outcome.status === 'error') reportFailure(outcome, request);
      announcement.value =
        outcome.status === 'error'
          ? outcome.message
          : outcome.status === 'stale'
            ? 'View updated; try again.'
            : '';
    } catch (error: unknown) {
      reportFailure(error, request);
      if (active && requestId === sequence) {
        announcement.value = error instanceof Error ? error.message : 'The canvas command failed.';
      }
    }
  }
  const unsubscribe = port.subscribe((snapshot) => {
    if (
      !active ||
      !sameCanvasAttachment(initial.address, snapshot.address) ||
      snapshot.revision <= state.value.revision
    )
      return;
    state.value = snapshot;
  });
  const app = createApp({
    render: () =>
      h(CanvasOverlay, {
        snapshot: state.value,
        capabilities: port.capabilities,
        send,
        announcement: announcement.value,
      }),
  });
  try {
    app.mount(element);
  } catch (error: unknown) {
    active = false;
    unsubscribe();
    reportFailure(error, null);
    throw error;
  }
  return {
    /** Make root teardown idempotent while invalidating pending command feedback. */
    dispose() {
      if (!active) return;
      active = false;
      unsubscribe();
      app.unmount();
    },
  };
}
