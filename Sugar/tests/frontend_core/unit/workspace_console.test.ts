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
/** Prove the console reads ComfyUI's output, shows it as the Python console did, and routes by mode. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { parseAnsiLine, plainAnsiLine } from '../../../frontend/comfyui/ui/console/AnsiText.js';
import { readLogLines } from '../../../frontend/comfyui/ui/console/ComfyLogFeed.js';
import {
  CONSOLE_LINE_LIMIT,
  ConsolePanel,
} from '../../../frontend/comfyui/ui/console/ConsolePanel.js';
import {
  COMFY_LOGS_COMMAND,
  WorkspaceConsole,
} from '../../../frontend/comfyui/ui/console/WorkspaceConsole.js';
import { sugarCubesCommandRegistration } from '../../../frontend/comfyui/ui/SugarCubesCommands.js';

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  localStorage.clear();
});

test('colours ANSI output in the Python console palette and drops other control sequences', () => {
  const line = '\u001b[32mloaded\u001b[0m in \u001b[1;31m3.2s\u001b[0m\u001b[2K';
  expect(parseAnsiLine(line)).toEqual([
    { text: 'loaded', color: '#33d17a', bold: false },
    { text: ' in ', color: null, bold: false },
    { text: '3.2s', color: '#f66151', bold: true },
  ]);
  expect(plainAnsiLine(line)).toBe('loaded in 3.2s');
});

test('reads every line of a log payload and ignores anything malformed', () => {
  expect(
    readLogLines({ entries: [{ t: 1, m: 'one\r\ntwo\n' }, { m: 3 }, null, { m: 'three' }] }),
  ).toEqual(['one', 'two', 'three']);
  expect(readLogLines(null)).toEqual([]);
});

test('the panel shows text literally, keeps its last lines, copies plain text and clears', () => {
  const copied: string[] = [];
  const panel = new ConsolePanel(
    document,
    (text) => copied.push(text),
    () => undefined,
  );
  document.body.append(panel.element);
  panel.append(['<img src=x onerror=alert(1)>', '\u001b[33mwarn\u001b[0m']);
  const output = panel.element.querySelector('.sc-console__output');
  expect(output?.querySelector('img')).toBeNull();
  expect(output?.textContent).toContain('<img src=x onerror=alert(1)>');
  expect(panel.element.querySelector('.sc-console__title')?.textContent).toBe('Comfy Console');
  panel.element.querySelector<HTMLButtonElement>('[aria-label="Copy all output"]')?.click();
  expect(copied).toEqual(['<img src=x onerror=alert(1)>\nwarn']);
  panel.append(
    Array.from({ length: CONSOLE_LINE_LIMIT + 5 }, (_, index) => `line ${String(index)}`),
  );
  expect(output?.childElementCount).toBe(CONSOLE_LINE_LIMIT);
  expect(output?.firstElementChild?.textContent).toBe('line 5');
  panel.element.querySelector<HTMLButtonElement>('[aria-label="Clear output"]')?.click();
  expect(output?.childElementCount).toBe(0);
  expect(panel.text()).toBe('');
});

/** A ComfyUI API client whose log feed is driven by the test. */
function logApi() {
  const listeners = new Set<(event: Event) => void>();
  const api = {
    subscribed: [] as boolean[],
    getRawLogs: async () => ({ entries: [{ m: 'kept line' }] }),
    subscribeLogs: async (enabled: boolean) => {
      api.subscribed.push(enabled);
    },
    addEventListener: (_type: string, listener: (event: Event) => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: (event: Event) => void) =>
      listeners.delete(listener),
    emit: (message: string) => {
      for (const listener of listeners)
        listener(new CustomEvent('logs', { detail: { entries: [{ m: message }] } }));
    },
  };
  return api;
}

test('inside the SugarSubstitute frame the console docks under the card column and follows ComfyUI', async () => {
  document.documentElement.setAttribute('data-sc-substitute-frame', '');
  const host = document.createElement('div');
  document.body.append(host);
  const api = logApi();
  const execute = jest.fn();
  const console = new WorkspaceConsole({
    document,
    app: { api, extensionManager: { command: { execute } } },
    getHost: () => host,
    report: (error) => {
      throw error;
    },
  });
  console.setup();
  console.toggle();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(host.querySelector('.sc-console')).not.toBeNull();
  expect(host.style.getPropertyValue('--sc-console-inset')).toBe('200px');
  api.emit('live line');
  expect(host.querySelector('.sc-console__output')?.textContent).toContain('kept line');
  expect(host.querySelector('.sc-console__output')?.textContent).toContain('live line');
  expect(api.subscribed).toEqual([true]);
  expect(execute).not.toHaveBeenCalled();

  console.toggle();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(host.querySelector('.sc-console')).toBeNull();
  expect(host.style.getPropertyValue('--sc-console-inset')).toBe('');
  expect(api.subscribed).toEqual([true, false]);
  console.dispose();
});

test('outside the frame the console command opens ComfyUI’s own Logs panel', () => {
  const execute = jest.fn(async () => undefined);
  const host = document.createElement('div');
  const console = new WorkspaceConsole({
    document,
    app: { api: logApi(), extensionManager: { command: { execute } } },
    getHost: () => host,
    report: (error) => {
      throw error;
    },
  });
  console.toggle();
  expect(execute).toHaveBeenCalledWith(COMFY_LOGS_COMMAND);
  expect(host.querySelector('.sc-console')).toBeNull();
  console.dispose();
});

test('the console is a registered command in the View menu, on ComfyUI’s console key', () => {
  const toggle = jest.fn();
  const registration = sugarCubesCommandRegistration({
    openCart: () => undefined,
    workspace: { masonry: { toggle }, canvas: { toggle }, console: { toggle } },
    projects: [],
  });
  const command = registration.commands.find((entry) => entry.id === 'SugarCubes.ToggleConsole');
  expect(command?.label).toBe('Toggle Console');
  command?.function();
  expect(toggle).toHaveBeenCalledTimes(1);
  expect(registration.menuCommands).toContainEqual({
    path: ['View'],
    commands: ['SugarCubes.ToggleConsole'],
  });
  expect(registration.keybindings).toContainEqual({
    combo: { key: '`', ctrl: true },
    commandId: 'SugarCubes.ToggleConsole',
  });
});
