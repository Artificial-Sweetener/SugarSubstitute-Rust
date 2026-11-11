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
/** Model the desktop host's single native surface and its one staging slot. */
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasObservationStatus } from '../../../integrations/contracts/CanvasChangeObservation.js';
import { finalViewport } from './NativeCanvasPreviewFixture.js';

type CommandType = CanvasSurfaceCommand['type'];

interface Ticket {
  readonly command: CanvasSurfaceCommand;
  readonly resolve: (result: CanvasSurfaceResult) => void;
}

/**
 * Answer native operations as the desktop host does for one surface with one staging slot.
 *
 * A prepared, uncommitted attachment holds the slot: a newer attachment's prepare is held until
 * that attachment commits or is released, and never answered otherwise. The host never rolls
 * back an abandoned staged attachment on its own. Operation types in `withheld` stay unanswered,
 * as a host still acquiring or presenting them would leave them, until the page cancels them.
 */
export function nativeCanvasStagingHost(options: { readonly overview?: boolean } = {}) {
  const calls: CanvasSurfaceCommand[] = [];
  const withheld = new Set<CommandType>();
  const waiting: Ticket[] = [];
  const stalled: Ticket[] = [];
  let staged: string | null = null;
  let committed: string | null = null;

  const answer = (ticket: Ticket, status: CanvasSurfaceResult['status']): void => {
    ticket.resolve(
      ticket.command.type === 'surface.viewport.request' && status === 'applied'
        ? { status, message: '', viewport: finalViewport }
        : { status, message: '' },
    );
  };
  /** Admit the oldest held prepare once the slot is free. */
  const admit = (): void => {
    if (staged) return;
    const next = waiting.shift();
    if (!next) return;
    staged = next.command.address.attachmentId;
    answer(next, 'ready');
  };
  const settle = (ticket: Ticket): void => {
    const { command } = ticket;
    const id = command.address.attachmentId;
    if (command.type === 'surface.prepare') {
      if (staged && staged !== id) waiting.push(ticket);
      else {
        staged = id;
        answer(ticket, 'ready');
      }
      return;
    }
    if (command.type === 'surface.release') {
      const index = waiting.findIndex((held) => held.command.address.attachmentId === id);
      if (index >= 0) waiting.splice(index, 1);
      if (staged === id) staged = null;
      if (committed === id) committed = null;
      answer(ticket, 'applied');
      admit();
      return;
    }
    if (withheld.has(command.type)) {
      stalled.push(ticket);
      return;
    }
    if (command.type === 'surface.commit') {
      if (staged !== id) {
        answer(ticket, 'stale');
        return;
      }
      staged = null;
      committed = id;
      answer(ticket, 'applied');
      admit();
      return;
    }
    answer(ticket, 'applied');
  };

  const host: CanvasSurfaceHost = {
    capabilities: {
      schema: 2,
      imageFormats: ['png'],
      previewFormats: [],
      viewportCommands: ['fit', 'native', 'zoom', 'pan'],
      viewportState: true,
      comparison: false,
      overview: options.overview ?? false,
      input: false,
    },
    presentation: {
      subscribe: () => ({
        ready: new Promise<CanvasObservationStatus>(() => {}),
        stop: () => {},
      }),
    },
    execute: (command, cancellation) => {
      calls.push(command);
      return new Promise<CanvasSurfaceResult>((resolve) => {
        const ticket: Ticket = { command, resolve };
        cancellation?.subscribe(() => {
          for (const queue of [waiting, stalled]) {
            const index = queue.indexOf(ticket);
            if (index >= 0) {
              queue.splice(index, 1);
              answer(ticket, 'cancelled');
            }
          }
        });
        settle(ticket);
      });
    },
  };
  return {
    host,
    calls,
    withheld,
    /** The attachment holding the staging slot, prepared and not yet committed or released. */
    staged: () => staged,
    committed: () => committed,
    /** Prepares the host holds behind the staged attachment. */
    held: () => waiting.map(({ command }) => command),
    /** Answer every withheld operation of one type, as a host that finishes that work would. */
    finish: (type: CommandType): void => {
      withheld.delete(type);
      for (const ticket of stalled.splice(0)) settle(ticket);
    },
    of: (attachmentId: string) =>
      calls.filter((call) => call.address.attachmentId === attachmentId).map(({ type }) => type),
  };
}
