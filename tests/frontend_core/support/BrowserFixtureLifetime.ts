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
/** Retire a browser fixture and its private HTTP connections as one finite test lifetime. */
import type { Server } from 'node:http';
import type { Browser } from '@playwright/test';

/** Stop accepting fixture requests and release active or idle sockets after observations finish. */
function closeFixtureServer(server: Server | undefined): Promise<void> {
  if (!server?.listening) return Promise.resolve();
  return new Promise<void>((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
    server.closeAllConnections();
  });
}

/** Attempt every owned release and preserve all teardown failures without leaking the other resource. */
export async function closeBrowserFixture(
  browser: Pick<Browser, 'close'> | undefined,
  server: Server | undefined,
): Promise<void> {
  const outcomes = await Promise.allSettled([closeFixtureServer(server), browser?.close()]);
  const causes = outcomes.flatMap((outcome): unknown[] =>
    outcome.status === 'rejected' ? [outcome.reason] : [],
  );
  if (causes.length) throw new AggregateError(causes, 'Browser fixture cleanup failed');
}
