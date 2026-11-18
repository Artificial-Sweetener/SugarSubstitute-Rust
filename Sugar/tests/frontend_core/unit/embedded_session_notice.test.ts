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
/** Prove the page shows a host's session notice once, and tells the host only when it was shown. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { EmbeddedPanelHost } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import {
  unsavedWorkWords,
  type UnsavedWork,
} from '../../../frontend/comfyui/ui/core/session_notice/UnsavedWorkNotice.js';
import { ToastService } from '../../../frontend/comfyui/ui/core/ToastService.js';

const session = '00000000-0000-4000-8000-000000000001';
const envelope = { protocol: 'sugarcubes.session-notice', version: 1, session };

afterEach(() => {
  jest.useRealTimers();
});

/** Start an embedded host whose description lists `protocols`; `reaches` says whether a notice is seen. */
function start(protocols: readonly string[], reaches = true) {
  jest.useFakeTimers();
  const sent: unknown[] = [];
  const channel: { receive: ((value: unknown) => void) | null } = { receive: null };
  const presented: UnsavedWork[] = [];
  const host = new EmbeddedPanelHost(
    {
      window,
      host: new HostDescriptionState(),
      presentation: new EmbeddedPresentation(),
      sessionNotice: (lost) => {
        presented.push(lost);
        return reaches;
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
  const receive = (value: unknown): void => channel.receive?.(value);
  receive({
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
  const notices = (): unknown[] =>
    sent.filter((value) => JSON.stringify(value).includes('sugarcubes.session-notice'));
  return { host, receive, presented, notices };
}

test('a host with session notices is watched, and a shown notice is answered once', () => {
  const { host, receive, presented, notices } = start(['sugarcubes.session-notice']);
  expect(notices()).toEqual([{ ...envelope, type: 'watch' }]);
  const notice = { ...envelope, type: 'not-saved', notice: 4, workspace: true, inputs: false };
  receive(notice);
  // The host may repeat a notice it still owes; the person sees it once.
  receive(notice);
  expect(presented).toEqual([{ workspace: true, inputs: false }]);
  expect(notices().slice(1)).toEqual([{ ...envelope, type: 'shown', notice: 4 }]);
  host.dispose();
});

test('a notice that did not reach the person is not answered, so the host still owes it', () => {
  const { host, receive, presented, notices } = start(['sugarcubes.session-notice'], false);
  receive({ ...envelope, type: 'not-saved', notice: 1, workspace: false, inputs: true });
  expect(presented).toEqual([{ workspace: false, inputs: true }]);
  expect(notices()).toEqual([{ ...envelope, type: 'watch' }]);
  host.dispose();
});

test('nothing to tell, another session and malformed notices show nothing', () => {
  const { host, receive, presented, notices } = start(['sugarcubes.session-notice']);
  receive({ ...envelope, type: 'none' });
  receive({ ...envelope, session: '00000000-0000-4000-8000-000000000002', type: 'not-saved' });
  receive({ ...envelope, type: 'not-saved', notice: 0, workspace: true, inputs: false });
  expect(presented).toEqual([]);
  expect(notices()).toHaveLength(1);
  host.dispose();
});

test('a host without the protocol is never asked', () => {
  const { host, notices } = start([]);
  expect(notices()).toEqual([]);
  host.dispose();
});

test('the notice names exactly what was lost', () => {
  expect(unsavedWorkWords({ workspace: true, inputs: false }).message).toBe(
    'Your latest workflow changes were not saved the last time the application closed.',
  );
  expect(unsavedWorkWords({ workspace: false, inputs: true }).message).toBe(
    'Your latest input canvas edits were not saved the last time the application closed.',
  );
  expect(unsavedWorkWords({ workspace: true, inputs: true }).message).toBe(
    'Your latest workflow changes and input canvas edits were not saved the last time the application closed.',
  );
});

test('a lasting toast carries no lifetime and reports whether the host showed it', () => {
  const add = jest.fn();
  const shown = new ToastService({ getToast: () => ({ add }) }).pushLasting(
    'warn',
    'Latest changes not saved',
    '<b>literal</b>',
  );
  expect(shown).toBe(true);
  expect(add).toHaveBeenCalledWith({
    severity: 'warn',
    summary: 'Latest changes not saved',
    detail: '<b>literal</b>',
  });
  expect(new ToastService({ getToast: () => null }).pushLasting('warn', 'a', 'b')).toBe(false);
});
