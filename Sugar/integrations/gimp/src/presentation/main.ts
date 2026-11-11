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
/** Render the local unavailable-server surface without depending on ComfyUI or duplicating SugarCubes UI. */
import { CompanionConnection } from '../application/CompanionConnection.js';
import { GimpPanelConnection } from '../infrastructure/GimpPanelConnection.js';
import { openNativeGimp } from '../infrastructure/TauriNativeGimp.js';
import type { NativeGimpSession } from '../application/NativeGimpSession.js';

/** Require each static shell element before binding its owned lifecycle. */
function element<T extends HTMLElement>(id: string, constructor: { new (...args: never[]): T }): T {
  const found = document.getElementById(id);
  if (!(found instanceof constructor)) throw new Error(`Missing companion shell element: ${id}`);
  return found;
}

const status = element('connection-status', HTMLHeadingElement);
const detail = element('connection-detail', HTMLParagraphElement);
const address = element('connection-address', HTMLParagraphElement);
const retry = element('retry', HTMLButtonElement);
const nativeDocument = element('native-document', HTMLParagraphElement);
let nativeSession: NativeGimpSession | null = null;
let panelConnection: GimpPanelConnection | null = null;
let closed = false;

/** Show launch-document identity even when the configured ComfyUI server is unavailable. */
async function connectNativeDocument(): Promise<void> {
  const native = await openNativeGimp(window, (error) => {
    console.error('SugarCubes GIMP native connection failed', { error });
  });
  if (closed) {
    native?.dispose();
    return;
  }
  nativeSession = native;
  panelConnection = new GimpPanelConnection(
    window,
    native ?? {
      execute: (command) =>
        Promise.resolve(
          command.type === 'context'
            ? {
                kind: 'context',
                hostId: 'unavailable',
                document: null,
                layers: [],
                primaryLayer: null,
                rgb: false,
              }
            : { kind: 'failure', status: 'unsupported', message: 'GIMP is not connected.' },
        ),
    },
    (error) => {
      console.error('SugarCubes GIMP panel connection failed', { error });
    },
  );
  if (!native) return;
  const context = await native.execute({ type: 'context' });
  if (nativeSession !== native) return;
  nativeDocument.hidden = false;
  nativeDocument.textContent =
    context.kind === 'context'
      ? (context.document?.name ?? 'The GIMP document has closed.')
      : 'The GIMP document is unavailable.';
}
const connection = new CompanionConnection(
  {
    connect: async (publish) => {
      await nativeReady;
      if (closed) return;
      if (!panelConnection) throw new Error('The GIMP connection did not initialize.');
      await panelConnection.connect(publish);
    },
    report: (error) => {
      console.error('SugarCubes companion connection failed', { error });
    },
  },
  (state) => {
    status.textContent =
      state.phase === 'connecting'
        ? 'Connecting…'
        : state.phase === 'connected'
          ? 'Connected'
          : 'ComfyUI is not connected';
    detail.textContent = state.message;
    address.textContent = state.address;
    retry.disabled = state.phase === 'connecting';
  },
);
retry.addEventListener('click', () => {
  void connection.retry();
});
window.addEventListener(
  'pagehide',
  () => {
    closed = true;
    panelConnection?.dispose();
    nativeSession?.dispose();
    nativeSession = null;
    connection.dispose();
  },
  { once: true },
);
const nativeReady = connectNativeDocument().catch((error: unknown) => {
  console.error('SugarCubes GIMP document metadata is unavailable', { error });
  if (!closed) {
    nativeDocument.hidden = false;
    nativeDocument.textContent = 'GIMP connection unavailable.';
  }
});
void connection.retry();
