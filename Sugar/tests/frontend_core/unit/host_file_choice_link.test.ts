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
/** Prove each file dialog question reaches the host and settles with the host's one answer. */
import { expect, test } from '@jest/globals';
import {
  HostFileChoice,
  HostFileChoiceLink,
} from '../../../frontend/comfyui/ui/core/file_choice/HostFileChoiceLink.js';

const session = 'session-0123456789abcdef';
const envelope = { protocol: 'sugarcubes.file-choice', version: 1, session };

/** A scripted host channel. */
function channel() {
  const sent: unknown[] = [];
  let receive: (value: unknown) => void = () => undefined;
  return {
    sent,
    host: (value: unknown) => receive(value),
    transport: {
      session,
      send: (value: unknown) => void sent.push(value),
      subscribe: (listener: (value: unknown) => void) => {
        receive = listener;
        return () => {
          receive = () => undefined;
        };
      },
    },
  };
}

/** A link to a scripted host, attached the way the embedded panel attaches it. */
function attached() {
  const host = channel();
  const dialog = new HostFileChoice();
  const link = new HostFileChoiceLink(host.transport, (error) => {
    throw error;
  });
  link.start();
  dialog.attach(link);
  return { host, dialog, link };
}

test('the dialog is there only while a host link is attached', async () => {
  const dialog = new HostFileChoice();
  expect(dialog.available).toBe(false);
  await expect(dialog.open('C:\\p', 'sugar')).rejects.toThrow('unavailable');
  const { dialog: hosted } = attached();
  expect(hosted.available).toBe(true);
  hosted.attach(null);
  expect(hosted.available).toBe(false);
});

test('open and save name their own requests, and each settles with its own answer', async () => {
  const { host, dialog } = attached();
  const opened = dialog.open('C:\\projects', 'sugar');
  const saved = dialog.save('C:\\projects\\Demo', 'Demo.sugar', 'sugar');
  expect(host.sent).toEqual([
    { ...envelope, type: 'open', request: 'open-1', folder: 'C:\\projects', extension: 'sugar' },
    {
      ...envelope,
      type: 'save',
      request: 'save-2',
      folder: 'C:\\projects\\Demo',
      extension: 'sugar',
      name: 'Demo.sugar',
    },
  ]);
  // Another session's answer, a malformed one and an unknown request settle nothing.
  host.host({
    ...envelope,
    session: 'session-ffffffffffffffff',
    type: 'dismissed',
    request: 'open-1',
  });
  host.host({ ...envelope, type: 'chosen', request: 'open-1', name: 'a.sugar' });
  host.host({ ...envelope, type: 'dismissed', request: 'open-9' });
  host.host({
    ...envelope,
    type: 'chosen',
    request: 'save-2',
    name: 'b.sugar',
    relative: 'Demo/b.sugar',
  });
  host.host({ ...envelope, type: 'dismissed', request: 'open-1' });
  expect(await saved).toEqual({ name: 'b.sugar', relative: 'Demo/b.sugar' });
  expect(await opened).toBeNull();
  // An answer is taken once.
  host.host({ ...envelope, type: 'chosen', request: 'open-1', name: 'a.sugar', relative: '' });
});

test('a dialog the host did not show fails with what happened', async () => {
  const { host, dialog } = attached();
  const busy = dialog.open('C:\\p', 'sugar');
  const missing = dialog.open('C:\\gone', 'sugar');
  host.host({ ...envelope, type: 'failed', request: 'open-1', reason: 'busy' });
  host.host({ ...envelope, type: 'failed', request: 'open-2', reason: 'unavailable' });
  await expect(busy).rejects.toThrow('Another file dialog is already open.');
  await expect(missing).rejects.toThrow('The file dialog could not be opened.');
});

test('a question outlived by its host fails instead of waiting forever', async () => {
  const { dialog, link } = attached();
  const waiting = dialog.open('C:\\p', 'sugar');
  link.dispose();
  await expect(waiting).rejects.toThrow('unavailable');
  await expect(link.ask({ kind: 'open', folder: 'C:\\p', extension: 'sugar' })).rejects.toThrow(
    'unavailable',
  );
});

test('a transport that cannot send retires the link and fails the question', async () => {
  const host = channel();
  const reported: unknown[] = [];
  const link = new HostFileChoiceLink(
    {
      ...host.transport,
      send: () => {
        throw new Error('closed');
      },
    },
    (error) => void reported.push(error),
  );
  link.start();
  await expect(link.ask({ kind: 'open', folder: 'C:\\p', extension: 'sugar' })).rejects.toThrow(
    'unavailable',
  );
  expect(reported).toHaveLength(1);
});
