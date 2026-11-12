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
/** Prove the Updates entry shows any update source's state in plain words and forwards requests. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  UpdatesSettingsPage,
  updateStatusText,
} from '../../../frontend/comfyui/ui/settings/UpdatesSettingsPage.js';
import {
  UpdateSourceSlot,
  type UpdateRequestOutcome,
  type UpdateSnapshot,
  type UpdateSource,
  type UpdateStatus,
} from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';

afterEach(() => {
  document.body.replaceChildren();
});

/** Deliver settled request promises to the page. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

const upToDate: UpdateSnapshot = {
  currentVersion: '0.4.2',
  channel: 'beta',
  lastSuccessfulCheck: 1762876800000,
  status: { state: 'up-to-date' },
  report: null,
  previousVersionRunning: false,
  workflowRunning: false,
  restartAfterWorkflow: false,
};

/** A scripted update source standing in for a host updater or SugarCubes' own backend. */
function source(applies: UpdateSource['applies'], initial: UpdateSnapshot | null = upToDate) {
  let snapshot = initial;
  const listeners = new Set<() => void>();
  let answer: (outcome: UpdateRequestOutcome) => void = () => undefined;
  const pending = (): Promise<UpdateRequestOutcome> =>
    new Promise((resolve) => {
      answer = resolve;
    });
  const value = {
    applies,
    read: () => snapshot,
    subscribe: (changed: () => void) => {
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    check: jest.fn(pending),
    apply: jest.fn(pending),
  } satisfies UpdateSource;
  return {
    value,
    publish(next: Partial<UpdateSnapshot>) {
      snapshot = { ...(snapshot ?? upToDate), ...next };
      for (const changed of [...listeners]) changed();
    },
    answer: (outcome: UpdateRequestOutcome) => answer(outcome),
    listeners: () => listeners.size,
  };
}

/** Mount the page against a slot and read what the user sees. */
function mount(slot: UpdateSourceSlot) {
  const report = jest.fn();
  const page = new UpdatesSettingsPage(document, slot, report, (epochMs) => `at ${epochMs}`);
  document.body.append(page.element);
  const text = (): string => page.element.textContent ?? '';
  const button = (label: string): HTMLButtonElement | undefined =>
    [...page.element.querySelectorAll('button')].find(
      (candidate) => candidate.textContent === label && !candidate.hidden,
    );
  const status = (): string => page.element.querySelector('[role="status"]')?.textContent ?? '';
  return { page, report, text, button, status };
}

test('shows the version, channel and last check, and a check result as it arrives', async () => {
  const slot = new UpdateSourceSlot();
  const host = source('host-restart');
  slot.attach(host.value);
  const view = mount(slot);
  expect(view.text()).toContain('0.4.2');
  expect(view.text()).toContain('Beta');
  expect(view.text()).toContain('at 1762876800000');
  expect(view.status()).toBe('SugarCubes is up to date.');
  view.button('Check for updates')?.click();
  expect(host.value.check).toHaveBeenCalledTimes(1);
  expect(view.button('Check for updates')?.disabled).toBe(true);
  host.publish({ status: { state: 'checking' } });
  expect(view.status()).toBe('Checking for updates…');
  host.answer({ accepted: true });
  await settle();
  host.publish({ status: { state: 'failed', during: 'check' }, lastSuccessfulCheck: null });
  expect(view.status()).toBe("Couldn't check for updates.");
  expect(view.text()).toContain('Not checked yet');
  expect(view.button('Check for updates')?.disabled).toBe(false);
  view.page.dispose();
  expect(host.listeners()).toBe(0);
});

test('a ready host update restarts now, or after the running workflow when one runs', async () => {
  const slot = new UpdateSourceSlot();
  const host = source('host-restart', {
    ...upToDate,
    status: { state: 'ready', version: '0.5.0' },
  });
  slot.attach(host.value);
  const view = mount(slot);
  expect(view.status()).toBe('Version 0.5.0 is ready. Restart to apply it.');
  expect(view.button('Restart after this workflow finishes')).toBeUndefined();
  host.publish({ workflowRunning: true });
  view.button('Restart after this workflow finishes')?.click();
  expect(host.value.apply).toHaveBeenLastCalledWith('after-workflow');
  host.answer({ accepted: true });
  await settle();
  host.publish({ restartAfterWorkflow: true });
  expect(view.text()).toContain('The restart will happen when the running workflow finishes.');
  expect(view.button('Restart after this workflow finishes')).toBeUndefined();
  view.button('Restart to apply')?.click();
  expect(host.value.apply).toHaveBeenLastCalledWith('now');
  host.answer({ accepted: false, reason: 'busy' });
  await settle();
  expect(view.text()).toContain('Updates are busy right now. Try again in a moment.');
  view.page.dispose();
});

test('a failed update shows its causes and that the previous version is running', () => {
  const slot = new UpdateSourceSlot();
  slot.attach(
    source('host-restart', {
      ...upToDate,
      status: { state: 'failed', during: 'update' },
      previousVersionRunning: true,
      report: {
        version: '0.5.0',
        phase: 'verify',
        causes: [{ message: 'The download did not match its signature.' }],
      },
    }).value,
  );
  const view = mount(slot);
  expect(view.status()).toBe("The update couldn't be installed.");
  expect(view.text()).toContain('The previous version is running.');
  expect([...view.page.element.querySelectorAll('li')].map((item) => item.textContent)).toEqual([
    'The download did not match its signature.',
  ]);
  view.page.dispose();
});

test("SugarCubes' own updater feeds the same entry and asks for a ComfyUI restart", () => {
  const slot = new UpdateSourceSlot();
  const own = source('comfyui-restart', {
    ...upToDate,
    status: { state: 'ready', version: '0.5.0' },
    workflowRunning: true,
  });
  const view = mount(slot);
  expect(view.status()).toBe('Updates are unavailable right now.');
  expect(view.button('Check for updates')).toBeUndefined();
  const release = slot.attach(own.value);
  expect(view.status()).toBe('Version 0.5.0 is ready. Apply it, then restart ComfyUI.');
  expect(view.button('Restart after this workflow finishes')).toBeUndefined();
  view.button('Apply update')?.click();
  expect(own.value.apply).toHaveBeenCalledWith('now');
  own.publish({ status: { state: 'restart-comfyui', version: '0.5.0' } });
  expect(view.status()).toBe('Version 0.5.0 will be installed when you restart ComfyUI.');
  own.publish({ status: { state: 'unmanaged', reason: 'git-checkout' } });
  expect(view.button('Check for updates')).toBeUndefined();
  release();
  expect(view.status()).toBe('Updates are unavailable right now.');
  view.page.dispose();
});

test('a copy that is never updated in place says why', () => {
  expect(updateStatusText({ state: 'unmanaged', reason: 'linked-folder' }, 'comfyui-restart')).toBe(
    "SugarCubes is installed from a linked folder, so it isn't updated here.",
  );
  expect(
    updateStatusText({ state: 'unmanaged', reason: 'not-registry-install' }, 'comfyui-restart'),
  ).toBe(
    "SugarCubes wasn't installed from ComfyUI Manager, so it isn't updated here. Reinstall it from ComfyUI Manager to get updates.",
  );
  expect(updateStatusText({ state: 'unmanaged', reason: 'git-checkout' }, 'comfyui-restart')).toBe(
    "This copy of SugarCubes is a git checkout, so it isn't updated here.",
  );
});

test('status words stay plain for every state', () => {
  const states: UpdateStatus[] = [
    { state: 'up-to-date' },
    { state: 'checking' },
    { state: 'available', version: '1.0.0' },
    { state: 'ready', version: '1.0.0' },
    { state: 'failed', during: 'check' },
    { state: 'failed', during: 'update' },
    { state: 'restart-comfyui', version: '1.0.0' },
    { state: 'unmanaged', reason: 'git-checkout' },
    { state: 'unmanaged', reason: 'local-changes' },
    { state: 'unmanaged', reason: 'linked-folder' },
    { state: 'unmanaged', reason: 'not-registry-install' },
  ];
  for (const status of states)
    for (const applies of ['host-restart', 'comfyui-restart'] as const)
      expect(updateStatusText(status, applies)).not.toMatch(/trial|candidate|quarantine|stag/i);
});
