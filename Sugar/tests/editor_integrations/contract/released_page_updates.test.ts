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
 * Prove a page that knows only `sugarcubes.updates` version 1, as every released SugarCubes page
 * does, keeps a working Updates entry with every recorded Substitute host.
 *
 * Such a page refuses a description that lists a protocol twice, reads only version 1 and
 * ignores capability names it does not know. Substitute rolls a broken SugarCubes update back to
 * the previous release, so this page must keep working against the newest host.
 */
import { expect, jest, test } from '@jest/globals';
import { isUpdatesHostMessage } from '../../../integrations/shared/UpdatesValidation.generated.js';
import { UpdatesHostLink } from '../../../frontend/comfyui/ui/core/updates/UpdatesHostLink.js';
import { recordedProfiles } from '../support/RecordedProfiles.js';

interface Message {
  readonly protocol?: unknown;
  readonly version?: unknown;
  readonly type?: unknown;
  readonly request?: unknown;
  readonly protocols?: readonly { readonly name: string; readonly version: number }[];
}

const profiles = recordedProfiles('substitute');

test.each(profiles.map((profile) => [profile.release, profile] as const))(
  'a version-one page gets a working Updates entry from Substitute %s',
  (_release, profile) => {
    const from = (side: 'host' | 'page') =>
      profile.transcript
        .filter((exchange) => exchange.from === side)
        .map((exchange) => exchange.message as Message);
    const description = from('host').find((message) => message.protocol === 'sugarcubes.host');
    const names = (description?.protocols ?? []).map((protocol) => protocol.name);
    expect(new Set(names).size).toBe(names.length);
    expect(description?.protocols).toContainEqual({ name: 'sugarcubes.updates', version: 1 });

    const updates = (message: Message) =>
      message.protocol === 'sugarcubes.updates' && message.version === 1;
    const asked = from('page').filter(updates);
    expect(asked.map((message) => message.type)).toEqual(['watch', 'check']);
    const answers = from('host').filter(updates);
    expect(answers.length).toBeGreaterThan(0);
    expect(answers.every(isUpdatesHostMessage)).toBe(true);
    expect(answers.map((message) => message.request ?? message.type)).toContain(asked[1]?.request);

    const listeners = new Set<(value: unknown) => void>();
    const link = new UpdatesHostLink(
      {
        session: profile.session ?? '',
        send: () => undefined,
        subscribe: (receive) => {
          listeners.add(receive);
          return () => listeners.delete(receive);
        },
      },
      jest.fn(),
      1,
    );
    link.start();
    for (const answer of answers) for (const listener of [...listeners]) listener(answer);
    const snapshot = link.read();
    expect(snapshot).not.toBeNull();
    expect(snapshot?.product).toBeNull();
    link.dispose();
  },
);
