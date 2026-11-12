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
/** Replay every recorded host release against the current page code. */
import { expect, test } from '@jest/globals';
import {
  HOST_PROFILE_SLOTS,
  HostProfileReplayFailure,
  PENDING_HOST_PROFILES,
  replayHostProfile,
  type RecordedHostProfile,
} from '../../../integrations/shared/conformance/HostProfiles.js';
import { DeclaredHost } from '../../../integrations/shared/HostDescription.js';
import { HOST_PROTOCOLS } from '../../../integrations/contracts/HostDescription.js';
import { recordedProfiles } from '../support/RecordedProfiles.js';

const recorded = HOST_PROFILE_SLOTS.flatMap((host) => recordedProfiles(host));

test('every host has a recorded profile unless it is listed as pending, and only then', () => {
  for (const host of HOST_PROFILE_SLOTS) {
    const pending = PENDING_HOST_PROFILES[host] !== undefined;
    expect([host, recordedProfiles(host).length > 0]).toEqual([host, !pending]);
  }
});

// Known pending item: the Substitute profile is recorded on the Rust side. This test passes while
// that profile is missing and fails the moment it lands, until the pending entry is removed.
test.failing('the Substitute host profile is recorded', () => {
  expect(PENDING_HOST_PROFILES.substitute).toBeUndefined();
});

test.each(recorded.map((profile) => [`${profile.host} ${profile.release}`, profile] as const))(
  'the current page still works with %s',
  async (_name, profile) => {
    expect(profile.schema).toBe(1);
    const declared = await replayHostProfile(profile);
    if (profile.channel === 'none') expect(declared).toBe(DeclaredHost.standalone);
    else expect(declared.serves(HOST_PROTOCOLS.panel)).toBe(true);
  },
);

/** The newest recorded profile of a host that answers over a panel channel. */
function panelProfile(): RecordedHostProfile {
  const profile = recorded.find(({ channel }) => channel === 'panel');
  if (!profile) throw new Error('No panel host profile is recorded.');
  return profile;
}

test('replay fails a profile whose recorded answers the current page cannot use', async () => {
  const profile = panelProfile();
  const broken = (edit: (message: Record<string, unknown>) => Record<string, unknown> | null) => ({
    ...profile,
    transcript: profile.transcript.flatMap((exchange) => {
      if (exchange.from !== 'host') return [exchange];
      const message = edit(exchange.message as Record<string, unknown>);
      return message ? [{ ...exchange, message }] : [];
    }),
  });
  await expect(
    replayHostProfile(
      broken((message) =>
        message.type === 'configure'
          ? { ...message, presentation: { ...(message.presentation as object), visualStyle: 'x' } }
          : message,
      ),
    ),
  ).rejects.toThrow(HostProfileReplayFailure);
  await expect(
    replayHostProfile(broken((message) => (message.type === 'description' ? null : message))),
  ).rejects.toThrow(HostProfileReplayFailure);
  await expect(
    replayHostProfile(
      broken((message) =>
        message.type === 'surface.capabilities.reply'
          ? { ...message, status: 'available', capabilities: { schema: 99 } }
          : message,
      ),
    ),
  ).rejects.toThrow(HostProfileReplayFailure);
});
