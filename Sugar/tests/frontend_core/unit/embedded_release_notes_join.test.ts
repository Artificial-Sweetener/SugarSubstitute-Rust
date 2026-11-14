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
/** Prove the page shows a host's release notes only when the host declares the protocol. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { EmbeddedPanelHost } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import type { HostReleaseNotesLink } from '../../../frontend/comfyui/ui/core/release_notes/HostReleaseNotesLink.js';

const session = '00000000-0000-4000-8000-000000000001';

afterEach(() => {
  jest.useRealTimers();
});

/** Start an embedded host whose description lists `protocols`, recording what it sends. */
function start(protocols: readonly string[]) {
  jest.useFakeTimers();
  const sent: unknown[] = [];
  const channel: { receive: ((value: unknown) => void) | null } = { receive: null };
  const attached: HostReleaseNotesLink[] = [];
  const detach = jest.fn();
  const host = new EmbeddedPanelHost(
    {
      window,
      host: new HostDescriptionState(),
      presentation: new EmbeddedPresentation(),
      releaseNotes: {
        attachHost: (link) => {
          attached.push(link);
          return detach;
        },
      },
      connected: () => true,
      report: jest.fn(),
    },
    {
      session,
      send: (value) => sent.push(value),
      subscribe: (listener) => {
        channel.receive = listener;
        return () => undefined;
      },
    },
  );
  host.setup();
  channel.receive?.({
    protocol: 'sugarcubes.host',
    version: 1,
    session,
    type: 'description',
    protocols: [
      { name: 'sugarcubes.panel', version: 3 },
      ...protocols.map((name) => ({ name, version: 1 })),
    ],
    capabilities: [],
  });
  return { host, sent, attached, detach };
}

test('a host that publishes release notes joins What is new and is watched', () => {
  const { host, sent, attached, detach } = start(['sugarcubes.release-notes']);
  expect(attached).toHaveLength(1);
  expect(sent).toContainEqual({
    protocol: 'sugarcubes.release-notes',
    version: 1,
    session,
    type: 'watch',
  });
  host.dispose();
  expect(detach).toHaveBeenCalledTimes(1);
});

test('a host without the protocol never joins', () => {
  const { host, sent, attached } = start([]);
  expect(attached).toHaveLength(0);
  expect(JSON.stringify(sent)).not.toContain('sugarcubes.release-notes');
  host.dispose();
});
