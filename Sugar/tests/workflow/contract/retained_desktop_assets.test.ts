/** @jest-environment node */
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
/** Verify retained animation facts through exact host metadata rather than stale names or MIME guesses. */
import { BrowserDesktopAssets } from '../../../frontend/comfyui/ui/workflow/infrastructure/BrowserDesktopAssets.js';
import {
  DesktopSessionCancellation,
  type DesktopSessionTransport,
} from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import { desktopSessionFixture } from '../support/desktop_session.js';

test('a cold retained WebP uses actual animation facts and releases its deadline', async () => {
  const shared = desktopSessionFixture();
  const original = shared.checkpoint.entries[0].outputs[0];
  const output = { ...original, mime: 'image/webp', assetDigest: original.digest };
  let timers = 0;
  const transport: DesktopSessionTransport = {
    scope: shared.checkpoint.scope,
    origin: shared.origin,
    webview: shared.fence.webview,
    send: async () => {},
    subscribe: () => () => {},
    assetUrl: () => `${shared.origin}/owned-asset`,
  };
  const calls: RequestInit[] = [];
  const request: typeof fetch = async (_url, options) => {
    calls.push(options ?? {});
    return new Response(null, {
      status: 200,
      headers: {
        'content-length': String(output.bytes),
        'content-type': output.mime,
        'x-sugarsubstitute-animated': '1',
      },
    });
  };
  const assets = new BrowserDesktopAssets(transport, request, {
    setTimeout: () => {
      timers++;
      return 1;
    },
    clearTimeout: () => {
      timers--;
    },
  });
  expect(
    await assets.presentation(
      output,
      { ...shared.fence.bindings[0], entryId: output.entryId },
      new DesktopSessionCancellation(),
    ),
  ).toEqual({ url: `${shared.origin}/owned-asset`, animated: true });
  expect(calls[0].method).toBe('HEAD');
  expect(timers).toBe(0);
});

test('changed or missing host facts fail instead of adopting an assumed animation kind', async () => {
  const shared = desktopSessionFixture();
  const original = shared.checkpoint.entries[0].outputs[0];
  const output = { ...original, assetDigest: original.digest };
  const transport: DesktopSessionTransport = {
    scope: shared.checkpoint.scope,
    origin: shared.origin,
    webview: shared.fence.webview,
    send: async () => {},
    subscribe: () => () => {},
    assetUrl: () => `${shared.origin}/owned-asset`,
  };
  const request: typeof fetch = async () =>
    new Response(null, {
      status: 200,
      headers: { 'content-length': String(output.bytes + 1), 'content-type': output.mime },
    });
  const assets = new BrowserDesktopAssets(transport, request, {
    setTimeout: () => 1,
    clearTimeout: () => {},
  });
  await expect(
    assets.presentation(
      output,
      { ...shared.fence.bindings[0], entryId: output.entryId },
      new DesktopSessionCancellation(),
    ),
  ).rejects.toThrow('exact recorded');
});
