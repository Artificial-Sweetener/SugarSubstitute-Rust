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
import { AboutSettingsPage } from '../../../frontend/comfyui/ui/settings/AboutSettingsPage.js';
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

/** Provide host-neutral metadata with markup-like runtime text. */
function fixture() {
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
  expect(f.page.element.querySelector('img,b')).toBeNull();
  const refresh = f.page.element.querySelector('button');
  refresh?.click();
  await settle();
  expect(f.source.load).toHaveBeenCalledTimes(2);
  expect(f.page.element.querySelector('button')).toBe(refresh);
  f.page.dispose();
});

test('open license through SugarCubes modal and recover after a failed metadata read', async () => {
  const f = fixture();
  jest.spyOn(f.source, 'load').mockRejectedValueOnce(new Error('offline'));
  f.page.refresh();
  await settle();
  expect(f.page.element.textContent).toContain('Refresh to try again');
  f.page.element.querySelector('button')?.click();
  await settle();
  expect(f.page.element.textContent).toContain('<b>SugarCubes</b>');
  const button = [...f.page.element.querySelectorAll('button')].find(
    (node) => node.textContent === 'View license',
  );
  button?.click();
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
  [...f.page.element.querySelectorAll('button')]
    .find((node) => node.textContent === 'View license')
    ?.click();
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
  [...f.page.element.querySelectorAll('button')]
    .find((node) => node.textContent === 'View license')
    ?.click();
  f.page.element.remove();
  if (!finish) throw new Error('License request did not start.');
  const resolve: (text: string) => void = finish;
  resolve('license');
  await settle();
  expect(document.querySelector('.sc-settings-license')).toBeNull();
  f.page.dispose();
});
