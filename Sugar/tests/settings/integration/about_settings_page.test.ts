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
/** Safeguard lazy About refresh, literal metadata and the shared license modal. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  AboutSettingsPage,
  type AboutHost,
} from '../../../frontend/comfyui/ui/settings/AboutSettingsPage.js';
import type {
  AboutInformation,
  AboutInformationSource,
} from '../../../frontend/comfyui/ui/settings/AboutInformation.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

/** Deliver asynchronous metadata and modal binding work. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

/** The page's button with one label. */
function button(page: AboutSettingsPage, label: string): HTMLButtonElement {
  const found = [...page.element.querySelectorAll('button')].find(
    (node) => node.textContent === label,
  );
  if (!found) throw new Error(`No ${label} button.`);
  return found;
}

/** Provide host-neutral metadata with markup-like runtime text. */
function fixture(host?: AboutHost) {
  const info: AboutInformation = {
    components: [
      {
        name: '<b>SugarCubes</b>',
        version: '<img src=x>',
        description: 'Description',
        author: 'Contributor',
        url: 'https://github.com/Artificial-Sweetener/SugarCubes',
      },
    ],
    warnings: [],
  };
  const source: AboutInformationSource = {
    noticesUrl: 'https://example.com/NOTICE.md',
    load: jest.fn<() => Promise<AboutInformation>>().mockResolvedValue(info),
    license: jest
      .fn<() => Promise<string>>()
      .mockResolvedValue('GNU AFFERO GENERAL PUBLIC LICENSE\n<b>literal</b>'),
  };
  const report = jest.fn<(error: unknown) => void>();
  const page = new AboutSettingsPage(
    document,
    source,
    { getDocument: () => document, getWindow: () => window },
    report,
    undefined,
    host,
  );
  document.body.append(page.element);
  return { page, source, report };
}

test('load lazily once, keep controls stable, and render runtime metadata literally', async () => {
  const f = fixture();
  expect(f.source.load).not.toHaveBeenCalled();
  f.page.refresh();
  f.page.refresh();
  await settle();
  f.page.refresh();
  expect(f.source.load).toHaveBeenCalledTimes(1);
  expect(f.page.element.textContent).toContain('<b>SugarCubes</b>');
  expect(f.page.element.querySelector('b,img:not(.sc-settings-about__logo)')).toBeNull();
  expect(f.page.element.textContent).toContain('Adobe Spectrum');
  const refresh = button(f.page, 'Refresh versions');
  expect(refresh.hidden).toBe(true);
  refresh.click();
  await settle();
  expect(f.source.load).toHaveBeenCalledTimes(2);
  expect(button(f.page, 'Refresh versions')).toBe(refresh);
  f.page.dispose();
});

test('open license through SugarCubes modal and recover after a failed metadata read', async () => {
  const f = fixture();
  jest.spyOn(f.source, 'load').mockRejectedValueOnce(new Error('offline'));
  f.page.refresh();
  await settle();
  expect(f.page.element.textContent).toContain('Refresh to try again');
  expect(button(f.page, 'Refresh versions').hidden).toBe(false);
  button(f.page, 'Refresh versions').click();
  await settle();
  expect(f.page.element.textContent).toContain('<b>SugarCubes</b>');
  button(f.page, 'Read AGPLv3').click();
  await settle();
  const modal = document.querySelector('.sc-settings-license');
  expect(modal?.textContent).toContain('GNU AFFERO GENERAL PUBLIC LICENSE');
  expect(modal?.querySelector('b')).toBeNull();
  const text = modal?.querySelector('pre');
  expect(text?.style.overflow).toBe('auto');
  expect(text?.style.maxHeight).not.toBe('');
  expect(document.activeElement).toBe(text);
  expect(f.report).toHaveBeenCalledTimes(1);
  f.page.dispose();
});

test('do not reopen a modal after the settings lifetime has been disposed', async () => {
  const f = fixture();
  let finish: ((text: string) => void) | null = null;
  jest.spyOn(f.source, 'license').mockReturnValue(
    new Promise<string>((resolve) => {
      finish = resolve;
    }),
  );
  button(f.page, 'Read AGPLv3').click();
  f.page.dispose();
  if (!finish) throw new Error('License request did not start.');
  const resolve: (text: string) => void = finish;
  resolve('license');
  await settle();
  expect(document.querySelector('.sc-settings-license')).toBeNull();
});

test('do not open a delayed license after its cached settings page leaves the dialog', async () => {
  const f = fixture();
  let finish: ((text: string) => void) | null = null;
  jest.spyOn(f.source, 'license').mockReturnValue(
    new Promise<string>((resolve) => {
      finish = resolve;
    }),
  );
  button(f.page, 'Read AGPLv3').click();
  f.page.element.remove();
  if (!finish) throw new Error('License request did not start.');
  const resolve: (text: string) => void = finish;
  resolve('license');
  await settle();
  expect(document.querySelector('.sc-settings-license')).toBeNull();
  f.page.dispose();
});

test('speaks for SugarCubes in a browser and for SugarSubstitute in its desktop', async () => {
  let state = { desktop: false, version: null as string | null };
  const listeners = new Set<() => void>();
  const f = fixture({
    read: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  f.page.refresh();
  await settle();
  const identity = f.page.element.querySelector('.sc-settings-about__identity strong');
  const versions = () =>
    [...f.page.element.querySelectorAll('.sc-settings-about__version strong')].map(
      (node) => node.textContent,
    );
  expect(identity?.textContent).toBe('SugarCubes');
  expect(versions()).toEqual(['<b>SugarCubes</b>']);
  state = { desktop: true, version: '0.9.0' };
  for (const listener of listeners) listener();
  expect(identity?.textContent).toBe('SugarSubstitute');
  expect(f.page.element.getAttribute('aria-label')).toBe('About SugarSubstitute');
  expect(versions()).toEqual(['SugarSubstitute', '<b>SugarCubes</b>']);
  const project = f.page.element.querySelector<HTMLAnchorElement>('a[href$="/issues"]');
  expect(project?.href).toBe('https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/issues');
  const card = f.page.element.querySelector<HTMLAnchorElement>('.sc-settings-about__version');
  expect(card?.title).toBe('Open SugarSubstitute project website');
  expect(card?.textContent).toContain('0.9.0');
  f.page.dispose();
  expect(listeners.size).toBe(0);
});
