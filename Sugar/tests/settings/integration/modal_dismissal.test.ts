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
/** Prove shared dismissal cancels each dialog without accepting edits or closing its parent. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ModalShell } from '../../../frontend/comfyui/ui/dialogs/ModalShell.js';
import { ModalService } from '../../../frontend/comfyui/ui/dialogs/ModalService.js';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';
import { VersionDialog } from '../../../frontend/comfyui/ui/dialogs/VersionDialog.js';
import { CubeAuthoringModal } from '../../../frontend/comfyui/ui/dialogs/CubeAuthoringModal.js';
import { TrackedPackManagerDialog } from '../../../frontend/comfyui/ui/settings/TrackedPackManagerDialog.js';
import { SaveNodePresetDialog } from '../../../frontend/comfyui/ui/editor_actions/presentation/SaveNodePresetDialog.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';
import { TestComfySettingsSelectRenderer } from '../support/ComfySettingsSelectTestRenderer.js';

const adapter = { getDocument: () => document, getWindow: () => window };
const shells: ModalShell[] = [];

afterEach(() => {
  for (const shell of shells.splice(0)) shell.close(null);
  document.body.replaceChildren();
});

/** Retain shell cleanup even when an assertion interrupts a pending result. */
function shell(): ModalShell {
  const value = new ModalShell({ adapter });
  shells.push(value);
  return value;
}

/** Locate the real close action rather than relying on footer order. */
function closeButton(root: ParentNode = document): HTMLButtonElement {
  const value = root.querySelector<HTMLButtonElement>('.sugarcubes-modal__close');
  if (!value) throw new Error('Missing header close button');
  return value;
}

const cases: readonly [string, () => Promise<unknown>, unknown][] = [
  ['confirmation', () => new ModalService({ adapter }).confirm(), false],
  ['alert', () => new ModalService({ adapter }).alert(), true],
  ['text input', () => new ModalService({ adapter }).promptText({ initialValue: 'draft' }), null],
  ['form', () => new ModalService({ adapter }).openForm(), null],
  ['selection', () => new ModalService({ adapter }).selectItem(), null],
  [
    'historical save',
    () => new ModalService({ adapter }).chooseHistoricalVersionSaveAction(),
    null,
  ],
  ['default review', () => new ModalService({ adapter }).reviewImplementationDefaults([]), null],
  [
    'version suggestions',
    () => new VersionDialog({ adapter }).open([{ default_alias: 'Cube' }]),
    false,
  ],
  ['tracked packs', () => new TrackedPackManagerDialog({ adapter }).open(), false],
  [
    'node preset',
    () => new SaveNodePresetDialog(document).open('Save preset', [GLOBAL_ASSOCIATION]),
    null,
  ],
  [
    'cube authoring',
    () =>
      new CubeAuthoringModal({
        adapter,
        settingsSelectRenderer: new TestComfySettingsSelectRenderer(),
      }).open({}),
    null,
  ],
];

for (const [name, open, cancelled] of cases) {
  test.each(['close', 'outside'] as const)(`${name}: %s resolves cancellation`, async (action) => {
    const result = open();
    const overlay = document.querySelector<HTMLElement>('.sugarcubes-modal-overlay.is-visible');
    if (!overlay) throw new Error('Missing visible modal');
    const button = closeButton(overlay);
    expect(button.type).toBe('button');
    expect(button.getAttribute('aria-label')).toBe('Close dialog');
    expect(overlay.querySelector('[role="dialog"][aria-modal="true"]')).not.toBeNull();
    if (action === 'close') button.click();
    else overlay.click();
    await expect(result).resolves.toEqual(cancelled);
    expect(overlay.classList.contains('is-visible')).toBe(false);
  });
}

test('content clicks do not cancel; dismissal restores focus and never confirms', async () => {
  const opener = document.createElement('button');
  document.body.append(opener);
  opener.focus();
  const modal = shell();
  const confirm = jest.fn();
  const result = modal.open({
    title: '<b>Literal</b>',
    cancelResult: 'cancelled',
    onConfirm: confirm,
  });
  modal.elements.content?.click();
  expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
  closeButton().click();
  await expect(result).resolves.toBe('cancelled');
  expect(confirm).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(opener);
});

test('a gesture starting inside the form cannot dismiss it when released on the backdrop', async () => {
  const modal = shell();
  const result = modal.open();
  modal.elements.content?.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  modal.elements.overlay?.click();
  expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
  modal.elements.overlay?.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  modal.elements.overlay?.click();
  await expect(result).resolves.toBeNull();
});

test('busy writes block every dismissal route and become dismissible after failure', async () => {
  const modal = shell();
  const result = modal.open();
  modal.setBusy(true);
  closeButton().click();
  modal.elements.cancelButton?.click();
  modal.elements.overlay?.click();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  expect(closeButton().disabled).toBe(true);
  expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
  modal.setBusy(false);
  modal.elements.overlay?.click();
  await expect(result).resolves.toBeNull();
});

test('only the top dialog responds to Escape, including reused shell stacking', async () => {
  const child = shell(),
    parent = shell();
  const first = child.open();
  child.close(null);
  await first;
  const parentResult = parent.open({ title: 'Parent' });
  const childResult = child.open({ title: 'Child' });
  parent.elements.overlay?.click();
  expect(parent.elements.overlay?.classList.contains('is-visible')).toBe(true);
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  await expect(childResult).resolves.toBeNull();
  expect(parent.elements.overlay?.classList.contains('is-visible')).toBe(true);
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  await expect(parentResult).resolves.toBeNull();
});

test('explicit escape and backdrop restrictions do not remove the deliberate close action', async () => {
  const modal = shell();
  const result = modal.open({ allowEscapeClose: false, allowOverlayClose: false });
  modal.elements.overlay?.click();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
  closeButton().click();
  await expect(result).resolves.toBeNull();
});

test.each(['dismissible', 'busy', 'restricted'] as const)(
  'Escape belongs to the %s child before native settings handle it',
  async (state) => {
    const nativeDismiss = jest.fn();
    const nativeKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && !event.defaultPrevented) nativeDismiss();
    };
    document.addEventListener('keydown', nativeKey);
    const modal = shell();
    const result = modal.open({ allowEscapeClose: state !== 'restricted' });
    if (state === 'busy') modal.setBusy(true);
    try {
      modal.elements.confirmButton?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );
      expect(nativeDismiss).not.toHaveBeenCalled();
      expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(
        state !== 'dismissible',
      );
    } finally {
      document.removeEventListener('keydown', nativeKey);
      modal.close(null);
      await result;
    }
  },
);

test('Escape dismisses an expanded native field before its enclosing SugarCubes dialog', async () => {
  const control = document.createElement('button');
  control.setAttribute('role', 'combobox');
  control.setAttribute('aria-expanded', 'true');
  const nativeDismiss = jest.fn();
  const nativeKey = (event: KeyboardEvent): void => {
    if (!event.defaultPrevented && event.key === 'Escape') nativeDismiss();
  };
  document.addEventListener('keydown', nativeKey);
  const modal = shell();
  const result = modal.open({ body: control });
  control.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    control.setAttribute('aria-expanded', 'false');
    event.preventDefault();
    event.stopPropagation();
  });
  try {
    control.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );
    expect(control.getAttribute('aria-expanded')).toBe('false');
    expect(nativeDismiss).not.toHaveBeenCalled();
    expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
  } finally {
    document.removeEventListener('keydown', nativeKey);
    modal.close(null);
    await result;
  }
});

test('Escape closes a shared context menu before its enclosing dialog', async () => {
  const modal = shell();
  const result = modal.open();
  const menu = new ContextMenu(document);
  menu.open({
    label: 'Dialog actions',
    trigger: modal.elements.confirmButton ?? null,
    anchor: { kind: 'pointer', x: 12, y: 12 },
    entries: [{ label: 'Action' }],
  });
  try {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );
    expect(menu.isOpen()).toBe(false);
    expect(modal.elements.overlay?.classList.contains('is-visible')).toBe(true);
    modal.elements.confirmButton?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );
    await expect(result).resolves.toBeNull();
  } finally {
    menu.close();
    modal.close(null);
  }
});
