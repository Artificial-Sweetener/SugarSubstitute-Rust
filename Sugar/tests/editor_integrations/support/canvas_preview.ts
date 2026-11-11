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
/** Fake native presentation boundaries while retaining the real shared session and observation owners. */
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfacePresentationChange,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasObservationStatus } from '../../../integrations/contracts/CanvasChangeObservation.js';
import { selectedPngCapabilities } from './canvas_surface.js';

export const session = 'preview-contract-session';

export const address = {
  sessionId: 'canvas:workflow:one',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;

export const preview = {
  generation: 0,
  promptId: 'prompt:one',
  sourceId: 'source:10:20',
  runOrder: 1,
  sequence: 1,
  token: 'preview:1',
} as const;

export const viewport = {
  mode: 'fit',
  physicalScale: 0.5,
  dimensions: { width: 640, height: 480 },
} as const;

export const capable: CanvasSurfaceCapabilities = {
  ...selectedPngCapabilities,
  viewportState: true,
  viewportCommands: ['fit', 'native', 'zoom', 'pan'],
  previewFormats: ['jpeg'],
};

export const begin: Extract<CanvasSurfaceCommand, { type: 'surface.preview.begin' }> = {
  type: 'surface.preview.begin',
  address,
  revision: 3,
  preview,
  byteLength: 3,
  format: 'jpeg',
};

/** Fake only native execution and observation behind the real shared authenticated session. */
export async function fixture(
  options: {
    capabilities?: CanvasSurfaceCapabilities;
    observation?: boolean;
    ready?: Promise<CanvasObservationStatus>;
    execute?(command: CanvasSurfaceCommand): Promise<CanvasSurfaceResult>;
  } = {},
) {
  const sent: CanvasMessage[] = [],
    errors: unknown[] = [],
    commands: CanvasSurfaceCommand[] = [];
  const listeners = new Set<(change: CanvasSurfacePresentationChange) => void>();
  let starts = 0,
    stops = 0;
  const server = new CanvasServer(session, {
    surface: {
      capabilities: options.capabilities ?? capable,
      execute: async (command): Promise<CanvasSurfaceResult> => {
        commands.push(command);
        if (options.execute) return await options.execute(command);
        return command.type === 'surface.viewport.request'
          ? { status: 'applied', message: '', viewport }
          : { status: command.type === 'surface.prepare' ? 'ready' : 'applied', message: '' };
      },
      ...(options.observation === false
        ? {}
        : {
            presentation: {
              subscribe: (listener: (change: CanvasSurfacePresentationChange) => void) => {
                starts++;
                listeners.add(listener);
                return {
                  ready: options.ready ?? Promise.resolve('observing' as const),
                  stop: () => {
                    stops++;
                    listeners.delete(listener);
                  },
                };
              },
            },
          }),
    },
    context: async () => ({
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
      },
      document: null,
    }),
    insert: async () => ({ status: 'unsupported', message: 'No layers.' }),
    send: (message) => client.receive(message),
    report: (error) => errors.push(error),
  });
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      server.receive(message);
    },
    later: () => () => {},
    report: (error) => errors.push(error),
  });
  await client.readContext();
  const host = client.surface;
  if (!host) throw new Error('Native surface discovery failed.');
  return {
    client,
    server,
    host,
    sent,
    errors,
    commands,
    listeners,
    starts: () => starts,
    stops: () => stops,
    close: () => {
      client.dispose();
      server.dispose();
    },
  };
}

/** Describe accepted native content independently of later ordinary mutation revisions. */
export function change(sequence = 1): CanvasSurfacePresentationChange {
  return {
    address,
    sequence,
    token: preview.token,
    leaseRevision: 3,
    status: 'presented',
    displayed: { type: 'preview', token: preview.token, leaseRevision: 3, viewport },
    message: '',
  };
}
