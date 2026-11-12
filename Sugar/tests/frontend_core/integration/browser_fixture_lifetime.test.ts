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
/** Prove browser fixture shutdown releases private HTTP requests and preserves cleanup failures. */
import { createServer, get, type ClientRequest, type IncomingMessage } from 'node:http';
import { closeBrowserFixture } from '../support/BrowserFixtureLifetime.js';

test('close an unfinished HTTP response instead of waiting for keep-alive or request timeouts', async () => {
  const server = createServer((_request, response) => {
    response.writeHead(200);
    response.write('pending');
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('Missing fixture listener');
  const errors: Error[] = [];
  let request: ClientRequest | undefined;
  let browserClosed = false;
  try {
    const response = await new Promise<IncomingMessage>((resolve, reject) => {
      request = get(`http://127.0.0.1:${String(address.port)}`, (response) => {
        response.on('error', (error: Error) => errors.push(error));
        response.once('data', () => resolve(response));
      });
      request.once('error', reject);
    });
    const closed = new Promise<void>((resolve) => response.once('close', () => resolve()));
    await closeBrowserFixture(
      {
        close: async () => {
          browserClosed = true;
        },
      },
      server,
    );
    await closed;
    expect(browserClosed).toBe(true);
    expect(server.listening).toBe(false);
    expect(response.destroyed).toBe(true);
    expect(response.complete).toBe(false);
    expect(errors.every((error) => 'code' in error && error.code === 'ECONNRESET')).toBe(true);
  } finally {
    request?.destroy();
    await closeBrowserFixture(undefined, server);
  }
});

test('release the HTTP listener even when browser shutdown fails', async () => {
  const server = createServer();
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const failure = new Error('Browser shutdown failed');
  await expect(
    closeBrowserFixture(
      {
        close: async () => {
          throw failure;
        },
      },
      server,
    ),
  ).rejects.toMatchObject({ errors: [failure] });
  expect(server.listening).toBe(false);
});
