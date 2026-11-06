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
/** Prepare a negotiated native surface while retaining the real session request sequence. */
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceHost,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasClient } from '../../../integrations/shared/CanvasClient.js';

export const selectedPngCapabilities: CanvasSurfaceCapabilities = {
  schema: 2,
  imageFormats: ['png'],
  viewportCommands: [],
  viewportState: false,
  previewFormats: [],
  comparison: false,
  overview: false,
  input: false,
};

/** Read a published surface without hiding a failed negotiation behind a type assertion. */
export function negotiatedSurface(client: CanvasClient): CanvasSurfaceHost {
  const surface = client.surface;
  if (!surface) throw new Error('The fixture surface was not negotiated.');
  return surface;
}

/** Respond to context and discovery while leaving surface operations under each test's control. */
export async function connectCanvasSurface(
  client: CanvasClient,
  sent: readonly CanvasMessage[],
  capabilities: CanvasSurfaceCapabilities = selectedPngCapabilities,
): Promise<void> {
  const pending = client.readContext();
  const context = sent.at(-1);
  if (context?.type !== 'context.request') throw new Error('Missing context request.');
  client.receive({
    ...context,
    type: 'context.reply',
    context: {
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: {
          displayOutput: true,
          insertImageLayer: false,
          positionImageLayer: false,
        },
      },
      document: null,
    },
  });
  await Promise.resolve();
  const discovery = sent.at(-1);
  if (discovery?.type !== 'surface.capabilities.request')
    throw new Error('Missing discovery request.');
  client.receive({
    protocol: discovery.protocol,
    version: discovery.version,
    session: discovery.session,
    requestId: discovery.requestId,
    type: 'surface.capabilities.reply',
    status: 'available',
    capabilities,
    message: '',
  });
  await pending;
}
