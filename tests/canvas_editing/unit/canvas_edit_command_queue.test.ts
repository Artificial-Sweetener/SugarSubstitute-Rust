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
/** Prove editing commands reach the host one at a time, in order, with drags coalesced. */
import { describe, expect, test } from '@jest/globals';
import { CanvasEditCommandQueue } from '../../../frontend/comfyui/ui/canvas_editing/application/CanvasEditCommandQueue.js';
import type {
  CanvasEditCommand,
  CanvasEditState,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasEditHostFixture, editState, settle } from '../support/CanvasEditHostFixture.js';

function queue(host: CanvasEditHostFixture) {
  const adopted: CanvasEditState[] = [];
  const failures: [unknown, CanvasEditCommand][] = [];
  const commands = new CanvasEditCommandQueue(host, {
    adopt: (state) => adopted.push(state),
    report: (error, command) => failures.push([error, command]),
  });
  return { commands, adopted, failures };
}

const size = (value: number): CanvasEditCommand => ({
  type: 'options',
  tool: 'brush',
  options: { size: value, hardness: 100, opacity: 100 },
});

describe('canvas edit command queue', () => {
  test('keeps one command in flight and replaces only unsent coalesced commands', async () => {
    const host = new CanvasEditHostFixture();
    host.hold = true;
    const { commands, adopted } = queue(host);
    const first = commands.send(size(10), 'options:brush');
    const superseded = commands.send(size(20), 'options:brush');
    const commit = commands.send({ type: 'commit' });
    const latest = commands.send(size(30), 'options:brush');
    await settle();
    expect(host.commands).toEqual([size(10)]);
    await expect(superseded).resolves.toBeNull();
    host.release();
    await expect(first).resolves.toEqual({ kind: 'state', state: host.state });
    await settle();
    host.release();
    await settle();
    host.release();
    await Promise.all([commit, latest]);
    // The latest value takes the superseded command's place, ahead of the later commit.
    expect(host.commands).toEqual([size(10), size(30), { type: 'commit' }]);
    expect(adopted).toHaveLength(3);
  });

  test('reports failures except cancellation and thrown transport errors', async () => {
    const host = new CanvasEditHostFixture();
    const { commands, failures } = queue(host);
    host.reply = (command) =>
      command.type === 'commit'
        ? { kind: 'failure', status: 'unsupported', message: 'Nothing to commit.' }
        : { kind: 'failure', status: 'cancelled', message: 'Cancelled.' };
    await commands.send({ type: 'commit' });
    await commands.send({ type: 'cancel' });
    expect(failures.map(([error, command]) => [(error as Error).message, command])).toEqual([
      ['Nothing to commit.', { type: 'commit' }],
    ]);
    host.reply = () => {
      throw new Error('Canvas edit connection closed.');
    };
    await expect(commands.send({ type: 'cancel' })).resolves.toBeNull();
    expect(failures).toHaveLength(2);
  });

  test('settles unsent commands without sending them once disposed', async () => {
    const host = new CanvasEditHostFixture(editState());
    host.hold = true;
    const { commands } = queue(host);
    void commands.send({ type: 'commit' });
    const pending = commands.send({ type: 'cancel' });
    await settle();
    commands.dispose();
    await expect(pending).resolves.toBeNull();
    await expect(commands.send({ type: 'commit' })).resolves.toBeNull();
    host.release();
    await settle();
    expect(host.commands).toEqual([{ type: 'commit' }]);
  });
});
