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

/**
 * Prove the Updates entry names the product whose version it shows when the current page runs
 * against the newest recorded Substitute host, over the page's real host-description and
 * updates path.
 */
import { afterEach, expect, jest, test } from '@jest/globals';
import { EmbeddedPanelHost } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import { UpdateSourceSlot } from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';
import { UpdatesSettingsPage } from '../../../frontend/comfyui/ui/settings/UpdatesSettingsPage.js';
import { recordedProfiles } from '../../editor_integrations/support/RecordedProfiles.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.useRealTimers();
});

test('the Updates entry names Substitute beside the version Substitute reports', () => {
  const profile = recordedProfiles('substitute').at(-1);
  if (!profile?.session) throw new Error('No recorded Substitute panel profile.');
  jest.useFakeTimers();
  const listeners = new Set<(value: unknown) => void>();
  const sent: unknown[] = [];
  const updates = new UpdateSourceSlot();
  const panel = new EmbeddedPanelHost(
    {
      window,
      host: new HostDescriptionState(),
      presentation: new EmbeddedPresentation(),
      updates,
      connected: () => true,
      report: jest.fn(),
    },
    {
      session: profile.session,
      send: (message) => sent.push(message),
      subscribe: (receive) => {
        listeners.add(receive);
        return () => listeners.delete(receive);
      },
    },
  );
  panel.setup();
  for (const exchange of profile.transcript)
    if (exchange.from === 'host') for (const listener of [...listeners]) listener(exchange.message);
  expect(sent).toContainEqual(
    expect.objectContaining({ protocol: 'sugarcubes.updates', version: 2, type: 'watch' }),
  );
  const entry = new UpdatesSettingsPage(document, updates, jest.fn());
  document.body.append(entry.element);
  const snapshot = updates.read()?.read();
  expect(snapshot?.product).toBe('SugarSubstitute');
  const status = entry.element.querySelector('[role="status"]')?.textContent ?? '';
  expect(status).toBe('SugarSubstitute is up to date.');
  expect(entry.element.textContent).toContain(snapshot?.currentVersion ?? '<none>');
  entry.dispose();
  panel.dispose();
});
