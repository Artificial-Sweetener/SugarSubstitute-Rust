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
/** Prove the What's new dialog shows release notes as readable, safe SugarCubes UI. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { WhatsNewModal } from '../../../frontend/comfyui/ui/dialogs/WhatsNewModal.js';
import { renderReleaseNotes } from '../../../frontend/comfyui/ui/dialogs/ReleaseNotesMarkdown.js';
import { WHATS_NEW_TEXT } from '../../../frontend/comfyui/ui/dialogs/WhatsNewText.js';
import { WhatsNewPresenter } from '../../../frontend/comfyui/ui/core/release_notes/WhatsNewPresenter.js';
import type {
  ReleaseNotesOutcome,
  ReleaseNotesSource,
} from '../../../frontend/comfyui/ui/core/release_notes/ReleaseNotesSource.js';
import { UpdatesSettingsPage } from '../../../frontend/comfyui/ui/settings/UpdatesSettingsPage.js';
import { UpdateSourceSlot } from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';

afterEach(() => {
  document.body.replaceChildren();
});

/** Let settled reads reach the dialog. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

const CHANGELOG = [
  '# [0.16.0](https://github.test/compare/sugarcubes-v0.15.3...sugarcubes-v0.16.0) (2026-11-10)',
  '',
  '',
  '### Features',
  '',
  "* **notes:** show what's new ([abc1234](https://github.test/commit/abc1234))",
  '  * nested `detail` item',
  '* <img src=x onerror=alert(1)> stays text',
  '* [a script link](javascript:alert(1))',
  '',
  'A closing paragraph.',
].join('\n');

/** A notes source whose reads the test answers. */
function source(product: string, currentVersion: string) {
  const answers: ((outcome: ReleaseNotesOutcome) => void)[] = [];
  const value = {
    read: () => ({ product, currentVersion }),
    subscribe: () => () => undefined,
    load: jest.fn(
      () =>
        new Promise<ReleaseNotesOutcome>((resolve) => {
          answers.push(resolve);
        }),
    ),
  } satisfies ReleaseNotesSource;
  return { value, answer: (outcome: ReleaseNotesOutcome) => answers.shift()?.(outcome) };
}

function seen(values: Record<string, string>) {
  return {
    read: (product: string) => values[product] ?? null,
    record: (product: string, version: string) => {
      values[product] = version;
    },
  };
}

const dialog = (): HTMLElement | null =>
  document.querySelector<HTMLElement>('.sugarcubes-whats-new-dialog');
const sections = (): HTMLElement[] => [
  ...document.querySelectorAll<HTMLElement>('.sugarcubes-whats-new__section'),
];

test('Markdown renders as headings, nested lists, links and code with markup kept literal', () => {
  const host = document.createElement('div');
  host.append(renderReleaseNotes(document, CHANGELOG));
  expect(host.querySelector('h4')?.textContent).toBe('0.16.0 (2026-11-10)');
  expect(host.querySelector('h6')?.textContent).toBe('Features');
  const links = [...host.querySelectorAll('a')];
  expect(links.map((link) => [link.textContent, link.href, link.target, link.rel])).toEqual([
    [
      '0.16.0',
      'https://github.test/compare/sugarcubes-v0.15.3...sugarcubes-v0.16.0',
      '_blank',
      'noopener noreferrer',
    ],
    ['abc1234', 'https://github.test/commit/abc1234', '_blank', 'noopener noreferrer'],
  ]);
  expect(host.querySelector('strong')?.textContent).toBe('notes:');
  expect(host.querySelector('ul ul li code')?.textContent).toBe('detail');
  expect(host.querySelector('img')).toBeNull();
  expect(host.textContent).toContain('<img src=x onerror=alert(1)> stays text');
  expect(host.textContent).toContain('a script link');
  expect(host.querySelector('p')?.textContent).toBe('A closing paragraph.');
});

test('an update opens one dialog that loads, then shows the notes', async () => {
  const sugar = source('SugarCubes', '0.16.0');
  new WhatsNewPresenter(
    new WhatsNewModal({ document }),
    seen({ SugarCubes: '0.15.3' }),
    jest.fn(),
  ).attach(sugar.value);
  expect(dialog()?.querySelector('.p-dialog-title')?.textContent).toBe("What's new");
  expect(sections()[0]?.getAttribute('aria-label')).toBe(
    'SugarCubes, updated from 0.15.3 to 0.16.0',
  );
  expect(sections()[0]?.textContent).toContain(WHATS_NEW_TEXT.loading);
  sugar.answer({ kind: 'notes', releases: [{ version: '0.16.0', notes: CHANGELOG }], omitted: 2 });
  await settle();
  expect(sections()[0]?.querySelector('h6')?.textContent).toBe('Features');
  expect(sections()[0]?.textContent).toContain('The notes of 2 earlier versions are not shown.');
  expect(sections()[0]?.textContent).not.toContain(WHATS_NEW_TEXT.loading);
});

test('offline shows the plain failure with Retry, and Retry reads again and shows the notes', async () => {
  const sugar = source('SugarCubes', '0.16.0');
  new WhatsNewPresenter(
    new WhatsNewModal({ document }),
    seen({ SugarCubes: '0.15.3' }),
    jest.fn(),
  ).attach(sugar.value);
  sugar.answer({ kind: 'failed', reason: 'unavailable' });
  await settle();
  expect(sections()[0]?.textContent).toContain(
    "Couldn't load what's new. Check your connection and try again.",
  );
  const retry = sections()[0]?.querySelector<HTMLButtonElement>('button');
  expect(retry?.textContent).toBe('Retry');
  retry?.click();
  expect(sugar.value.load).toHaveBeenCalledTimes(2);
  expect(sections()[0]?.textContent).toContain(WHATS_NEW_TEXT.loading);
  sugar.answer({ kind: 'notes', releases: [{ version: '0.16.0', notes: '* fixed' }], omitted: 0 });
  await settle();
  expect(sections()[0]?.querySelector('li')?.textContent).toBe('fixed');
  expect(sections()[0]?.querySelector('button')).toBeNull();
});

test('SugarCubes and the host updated together share one dialog in labelled sections', async () => {
  const sugar = source('SugarCubes', '0.16.0');
  const host = source('SugarSubstitute', '0.5.0');
  const presenter = new WhatsNewPresenter(
    new WhatsNewModal({ document }),
    seen({ SugarCubes: '0.15.3', SugarSubstitute: '0.3.0' }),
    jest.fn(),
  );
  presenter.attach(sugar.value);
  presenter.attach(host.value);
  sugar.answer({ kind: 'notes', releases: [{ version: '0.16.0', notes: '* cubes' }], omitted: 0 });
  host.answer({ kind: 'failed', reason: 'not-published' });
  await settle();
  expect(document.querySelectorAll('.sugarcubes-whats-new-dialog')).toHaveLength(1);
  expect(sections().map((section) => section.querySelector('h3')?.textContent)).toEqual([
    'SugarCubes, updated from 0.15.3 to 0.16.0',
    'SugarSubstitute, updated from 0.3.0 to 0.5.0',
  ]);
  expect(sections()[1]?.textContent).toContain(
    'No release notes are published for SugarSubstitute 0.5.0 yet.',
  );
});

test('the Updates entry opens the same dialog, and closing it lets it open again', async () => {
  const sugar = source('SugarCubes', '0.16.0');
  const presenter = new WhatsNewPresenter(
    new WhatsNewModal({ document }),
    seen({ SugarCubes: '0.16.0' }),
    jest.fn(),
  );
  presenter.attach(sugar.value);
  expect(dialog()).toBeNull();
  const page = new UpdatesSettingsPage(document, new UpdateSourceSlot(), jest.fn(), undefined, () =>
    presenter.open(),
  );
  document.body.append(page.element);
  const button = [...page.element.querySelectorAll('button')].find(
    (candidate) => candidate.textContent === "What's new",
  );
  expect(button?.hidden).toBe(false);
  button?.click();
  expect(sections().map((section) => section.querySelector('h3')?.textContent)).toEqual([
    'SugarCubes 0.16.0',
  ]);
  sugar.answer({ kind: 'notes', releases: [{ version: '0.16.0', notes: '* x' }], omitted: 0 });
  await settle();
  dialog()?.querySelector<HTMLButtonElement>('.sugarcubes-modal__close')?.click();
  await settle();
  expect(dialog()?.closest<HTMLElement>('.sugarcubes-modal-overlay')?.style.display).toBe('none');
  button?.click();
  expect(dialog()?.closest<HTMLElement>('.sugarcubes-modal-overlay')?.style.display).toBe('flex');
  expect(sugar.value.load).toHaveBeenCalledTimes(1);
  page.dispose();
});

test('without a What is new owner the Updates entry offers no such button', () => {
  const page = new UpdatesSettingsPage(document, new UpdateSourceSlot(), jest.fn());
  const button = [...page.element.querySelectorAll('button')].find(
    (candidate) => candidate.textContent === "What's new",
  );
  expect(button?.hidden).toBe(true);
});

test('long notes scroll between a title bar and response buttons that stay in place', async () => {
  const modal = new WhatsNewModal({ document });
  modal.show(
    [
      {
        key: 'sugarcubes',
        product: 'SugarCubes',
        currentVersion: '0.16.0',
        updatedFrom: '0.15.3',
        status: { kind: 'loading' },
      },
    ],
    { retry: jest.fn(), closed: jest.fn() },
  );
  await settle();
  const dialog = document.querySelector<HTMLElement>('.sugarcubes-whats-new-dialog');
  if (!dialog) throw new Error("The What's new dialog did not open.");
  const [header, content, footer] = [...dialog.children] as HTMLElement[];
  expect(header?.querySelector('.sugarcubes-modal__close')).not.toBeNull();
  expect([...(footer?.querySelectorAll('button') ?? [])].at(-1)?.textContent).toBe(
    WHATS_NEW_TEXT.close,
  );
  // The dialog itself never scrolls, so its first and last rows cannot leave the view.
  expect(getComputedStyle(dialog).overflow).toBe('hidden');
  expect(getComputedStyle(dialog).flexDirection).toBe('column');
  expect(getComputedStyle(header as HTMLElement).flex).toBe('0 0 auto');
  expect(getComputedStyle(footer as HTMLElement).flex).toBe('0 0 auto');
  expect(getComputedStyle(content as HTMLElement).overflow).toBe('auto');
  expect(getComputedStyle(content as HTMLElement).minHeight).toBe('0');
});
