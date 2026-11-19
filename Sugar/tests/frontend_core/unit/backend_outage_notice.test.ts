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
/** Prove the person is told when the host's ComfyUI stops and is the one who starts it again. */
import { expect, jest, test } from '@jest/globals';
import { BackendOutageNotice } from '../../../frontend/comfyui/ui/core/backend/BackendOutageNotice.js';
import { HostBackendLink } from '../../../frontend/comfyui/ui/core/backend/HostBackendLink.js';

const SESSION = 'session-0123456789abcdef';

function fixture(answer: boolean | Promise<boolean> = true) {
  const shown: string[] = [];
  const confirm = jest.fn(async (title: string, _message: string, agree: string) => {
    shown.push(`ask ${title} / ${agree}`);
    return answer;
  });
  const notice = new BackendOutageNotice({
    confirm,
    toast: (severity, summary) => void shown.push(`${severity} ${summary}`),
    lastingToast: (severity, summary, detail) =>
      void shown.push(`lasting ${severity} ${summary}: ${detail}`),
    report: (error) => void shown.push(`error ${String(error)}`),
  });
  let receive: ((value: unknown) => void) | null = null;
  const sent: unknown[] = [];
  const link = new HostBackendLink(
    {
      session: SESSION,
      send: (message) => void sent.push(message),
      subscribe: (listener) => {
        receive = listener;
        return () => (receive = null);
      },
    },
    notice,
    (error) => void shown.push(`error ${String(error)}`),
  );
  link.start();
  const envelope = { protocol: 'sugarcubes.backend', version: 1, session: SESSION };
  const host = (revision: number, status: string) =>
    receive?.({ ...envelope, type: 'state', revision, status });
  const settle = async () => {
    for (let turn = 0; turn < 4; turn += 1) await Promise.resolve();
  };
  return { notice, link, shown, sent, host, envelope, settle, confirm };
}

test('a running backend says nothing, and the page asks the host once to watch it', () => {
  const f = fixture();
  f.host(0, 'running');
  expect(f.sent).toEqual([{ ...f.envelope, type: 'watch' }]);
  expect(f.shown).toEqual([]);
  expect(f.notice.canRestart).toBe(false);
});

test('a stopped backend is announced, offered for restart, restarted on a yes and welcomed back', async () => {
  const f = fixture(true);
  f.host(1, 'stopped');
  expect(f.shown).toEqual(['lasting warn ComfyUI stopped: It cannot be restarted yet.']);
  f.host(2, 'retry-ready');
  expect(f.notice.canRestart).toBe(true);
  await f.settle();
  expect(f.shown.at(-1)).toBe('ask ComfyUI stopped / Restart ComfyUI');
  expect(f.sent.at(-1)).toEqual({ ...f.envelope, type: 'retry', revision: 2 });
  expect(f.notice.canRestart).toBe(false);
  f.host(3, 'retrying');
  f.host(4, 'running');
  expect(f.shown.slice(-2)).toEqual([
    'info Restarting ComfyUI…',
    'success ComfyUI is running again',
  ]);
});

test('declining leaves ComfyUI stopped and the restart in the application menu', async () => {
  const f = fixture(false);
  f.host(1, 'retry-ready');
  await f.settle();
  expect(f.sent).toHaveLength(1);
  expect(f.shown.at(-1)).toBe(
    'lasting warn ComfyUI stopped: Choose Restart ComfyUI in the application menu when you want it back.',
  );
  expect(f.notice.canRestart).toBe(true);
  f.notice.restart();
  f.notice.restart();
  expect(f.sent.slice(1)).toEqual([{ ...f.envelope, type: 'retry', revision: 1 }]);
});

test('an answer given after the backend moved on restarts nothing', async () => {
  let answer: (value: boolean) => void = () => undefined;
  const f = fixture(new Promise<boolean>((resolve) => (answer = resolve)));
  f.host(1, 'retry-ready');
  f.host(2, 'retrying');
  answer(true);
  await f.settle();
  expect(f.sent).toHaveLength(1);
});

test('repeated, stale, malformed and foreign states are ignored', async () => {
  const f = fixture(false);
  f.host(2, 'retry-ready');
  f.host(2, 'retry-ready');
  f.host(1, 'stopped');
  f.host(3, 'exploded');
  await f.settle();
  expect(f.confirm).toHaveBeenCalledTimes(1);
  f.link.dispose();
  f.host(9, 'running');
  expect(f.shown.filter((line) => line.startsWith('success'))).toEqual([]);
});
