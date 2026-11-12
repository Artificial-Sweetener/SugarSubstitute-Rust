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
/**
 * Connect a host built on the shared session owners to the conformance kit in memory.
 *
 * Hosts that compose `PanelHostSession` and `CanvasServer`, such as the GIMP and Photoshop
 * panels, run the kit through this channel without their native windows. Each handler receives
 * every page message and answers through `send`; the channel delivers host messages to the kit
 * synchronously, in order.
 */
import type { HostDescriptionContent } from '../../contracts/HostDescription.js';
import type { HostDescriptionMessage } from '../../contracts/HostProtocol.generated.js';
import type { PanelMessage, PanelPresentation } from '../../contracts/PanelProtocol.generated.js';
import { PanelHostSession } from '../PanelSession.js';
import type { ConformanceHostTransport } from './HostConformance.js';

/** One host protocol owner on the channel: it reads page messages and answers through `send`. */
export interface InMemoryHostHandler {
  /** @returns Whether the handler owned the message. */
  receive(value: unknown): boolean;
}

/** The host side the channel composes: the panel session and any further protocol owners. */
export interface InMemoryHostOptions {
  readonly session: string;
  readonly description: HostDescriptionContent;
  readonly presentation: PanelPresentation;
  /** Build further owners, such as a `CanvasServer`, that answer through the given send. */
  handlers?(send: (message: unknown) => void): readonly InMemoryHostHandler[];
}

/** Create the kit's transport over a host composed from the shared session owners. */
export function inMemoryHostChannel(options: InMemoryHostOptions): ConformanceHostTransport {
  const listeners = new Set<(value: unknown) => void>();
  const send = (message: unknown): void => {
    for (const listener of [...listeners]) listener(message);
  };
  const panel = new PanelHostSession(
    options.session,
    options.description,
    options.presentation,
    {
      send: (message: PanelMessage | HostDescriptionMessage) => {
        send(message);
      },
      present: () => undefined,
    },
    0,
  );
  const handlers = options.handlers?.(send) ?? [];
  return {
    send: (message) => {
      if (panel.receive(message, 0)) return;
      for (const handler of handlers) if (handler.receive(message)) return;
    },
    subscribe: (receive) => {
      listeners.add(receive);
      return () => listeners.delete(receive);
    },
  };
}
