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
/** Relay real public editor protocols to the native SDK while qualifying external browser rendering. */
import { PanelHostSession, type PanelState } from '../../../integrations/shared/PanelSession.js';
import { CanvasMaskServer } from '../../../integrations/shared/CanvasMaskServer.js';
import { GimpCanvasBridge } from '../../../integrations/gimp/src/infrastructure/GimpCanvasBridge.js';
import { GimpCanvasMasks } from '../../../integrations/gimp/src/infrastructure/GimpCanvasMasks.js';
import { GIMP_HOST_DESCRIPTION } from '../../../integrations/gimp/src/infrastructure/GimpPanelConnection.js';
import { openNativeGimp } from '../../../integrations/gimp/src/infrastructure/TauriNativeGimp.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import type { CanvasEncodedImage } from '../../../integrations/contracts/CanvasProtocol.generated.js';

/** Keep the test-only browser relay replaceable without replacing any native pixels or protocol owners. */
export interface NativeBrowserProofBridge {
  receive(value: unknown): void;
  read(): { readonly state: PanelState; readonly errors: readonly string[] };
  insertFixtureLayer(image: CanvasEncodedImage, name: string): Promise<void>;
  dispose(): void;
}

/** Run the production GIMP servers and shared panel negotiation on an actual local Tauri renderer. */
export async function connectNativeBrowserProof(
  host: object,
  session: string,
  send: (value: unknown) => void,
): Promise<NativeBrowserProofBridge> {
  const errors: string[] = [];
  const report = (error: unknown): void => {
    errors.push(error instanceof Error ? error.message : String(error));
  };
  const native = await openNativeGimp(host, report);
  if (!native) throw new Error('Browser qualification requires the actual GIMP native adapter.');
  const canvas = new GimpCanvasBridge(session, native, send, report);
  const maskHost = new GimpCanvasMasks(native, report);
  const masks = new CanvasMaskServer(session, {
    changes: maskHost.changes,
    execute: (command) => maskHost.execute(command),
    send,
    report,
  });
  let state: PanelState = 'loading';
  const panel = new PanelHostSession(
    session,
    GIMP_HOST_DESCRIPTION,
    {
      pluginMode: true,
      visualStyle: 'gimp',
      accentColor: null,
      resizeCornerSize: 0,
    },
    {
      send,
      present: (value) => {
        state = value;
      },
    },
    performance.now(),
  );
  const timer = setInterval(() => panel.tick(performance.now()), 1000);
  return {
    receive: (value) => {
      if (panel.receive(value, performance.now()) || canvas.receive(value)) return;
      masks.receive(value);
    },
    read: () => ({ state, errors: [...errors] }),
    insertFixtureLayer: async (image, name) => {
      const editSession = crypto.randomUUID();
      const bridge = new GimpCanvasBridge(
        editSession,
        native,
        (value) => client.receive(value),
        report,
      );
      const client = new CanvasClient(editSession, {
        send: (value) => {
          bridge.receive(value);
        },
        later: (milliseconds, callback) => {
          const timer = setTimeout(callback, milliseconds);
          return () => clearTimeout(timer);
        },
        report,
      });
      try {
        const { document } = await client.readContext();
        if (!document) throw new Error('Missing native fixture document.');
        const result = await client.insertImage({
          document,
          image,
          layerName: name,
          position: null,
        });
        if (result.status !== 'inserted')
          throw new Error('Native fixture insertion did not commit.');
      } finally {
        client.dispose();
        bridge.dispose();
      }
    },
    dispose: () => {
      clearInterval(timer);
      canvas.dispose();
      masks.dispose();
      maskHost.dispose();
      native.dispose();
    },
  };
}
