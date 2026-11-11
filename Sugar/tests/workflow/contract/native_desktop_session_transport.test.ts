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
/** Validate the actual native bootstrap adapter before session or retained-media effects. */
import { createNativeDesktopSessionTransport } from '../../../frontend/comfyui/ui/workflow/infrastructure/NativeDesktopSessionTransport.js';
import { desktopSessionFixture } from '../support/desktop_session.js';
import type { SessionMessage } from '../../../integrations/contracts/SessionProtocol.generated.js';

afterEach(() => {
  Reflect.deleteProperty(window, 'sugarcubesSessionHost');
});

test('adapts exact current-origin scope and awaits native message admission with explicit teardown', async () => {
  const shared = desktopSessionFixture();
  const sent: SessionMessage[] = [];
  let stopped = false;
  const origin = window.location.origin;
  Object.defineProperty(window, 'sugarcubesSessionHost', {
    configurable: true,
    value: {
      scope: shared.checkpoint.scope,
      origin,
      webview: shared.fence.webview,
      send: async (message: SessionMessage) => {
        sent.push(message);
      },
      subscribe: () => () => {
        stopped = true;
      },
    },
  });
  const transport = createNativeDesktopSessionTransport(window)!;
  const message: SessionMessage = {
    protocol: 'sugarcubes.desktop-session',
    version: 1,
    requestId: shared.checkpoint.id,
    scope: transport.scope,
    origin,
    operation: {
      kind: 'ready',
      capabilities: {
        capture: true,
        materialize: true,
        chunkTransfer: true,
        artifactResolution: false,
      },
      committed: true,
      workflowPersistence: true,
      fence: shared.fence,
    },
  };
  await transport.send(message);
  expect(sent).toEqual([message]);
  transport.subscribe(() => {})();
  expect(stopped).toBe(true);
  await expect(transport.send({ ...message, origin: 'https://untrusted.invalid' })).rejects.toThrow(
    'untrusted-origin',
  );
});

test('retained URL capability stays exact to origin revocable namespace and output identity', () => {
  const shared = desktopSessionFixture();
  const outputId = shared.checkpoint.entries[0].outputs[0].id;
  const origin = window.location.origin;
  const query = new URLSearchParams({ binding: JSON.stringify(shared.fence.bindings[0]) });
  let url = `${origin}/__sugarsubstitute/session-media/v1/${'a'.repeat(32)}/${outputId}?${query}`;
  Object.defineProperty(window, 'sugarcubesSessionHost', {
    configurable: true,
    value: {
      scope: shared.checkpoint.scope,
      origin,
      webview: shared.fence.webview,
      send: async () => {},
      subscribe: () => () => {},
      media: { version: 1, assetUrl: () => url },
    },
  });
  const transport = createNativeDesktopSessionTransport(window)!;
  expect(transport.assetUrl?.(outputId, shared.fence.bindings[0])).toBe(url);
  url = `https://untrusted.invalid/__sugarsubstitute/session-media/v1/${'a'.repeat(32)}/${outputId}`;
  expect(() => transport.assetUrl?.(outputId, shared.fence.bindings[0])).toThrow('escaped');
});

test('ordinary browser needs no native bridge, while malformed injection fails closed', () => {
  expect(createNativeDesktopSessionTransport(window)).toBeNull();
  Object.defineProperty(window, 'sugarcubesSessionHost', {
    configurable: true,
    value: { origin: 'https://wrong.invalid' },
  });
  expect(() => createNativeDesktopSessionTransport(window)).toThrow('invalid');
});
