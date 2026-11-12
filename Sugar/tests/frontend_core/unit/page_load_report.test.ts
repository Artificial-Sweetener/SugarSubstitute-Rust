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
/**
 * @jest-environment jsdom
 */
/** Prove the page reports whether SugarCubes loaded, so a broken version is put back. */
import { afterEach, expect, test } from '@jest/globals';
import {
  SETUP_OUTCOME_ATTRIBUTE,
  SUGARCUBES_EXTENSION_NAME,
  announceSetupOutcome,
  announcedSetup,
  reportPageLoad,
} from '../../../frontend/comfyui/ui/core/updates/PageLoadReport.js';

afterEach(() => document.documentElement.removeAttribute(SETUP_OUTCOME_ATTRIBUTE));

test('reports a failed load when the main extension never registered', () => {
  const reports: boolean[] = [];
  reportPageLoad({
    registeredExtensions: ['Other'],
    document,
    report: (loaded) => reports.push(loaded),
  });
  expect(reports).toEqual([false]);
});

test('waits for the main setup to end and reports its outcome once', () => {
  const reports: boolean[] = [];
  reportPageLoad({
    registeredExtensions: [SUGARCUBES_EXTENSION_NAME],
    document,
    report: (loaded) => reports.push(loaded),
  });
  expect(reports).toEqual([]);
  document.dispatchEvent(new CustomEvent('sugarcubes-setup-settled', { detail: 'malformed' }));
  expect(reports).toEqual([]);
  announceSetupOutcome(document, false);
  announceSetupOutcome(document, true);
  expect(reports).toEqual([false]);
});

test('reads an outcome announced before the report started', () => {
  announceSetupOutcome(document, true);
  const reports: boolean[] = [];
  reportPageLoad({
    registeredExtensions: [SUGARCUBES_EXTENSION_NAME],
    document,
    report: (loaded) => reports.push(loaded),
  });
  expect(reports).toEqual([true]);
});

test('releasing a pending report stops listening', () => {
  const reports: boolean[] = [];
  const release = reportPageLoad({
    registeredExtensions: [SUGARCUBES_EXTENSION_NAME],
    document,
    report: (loaded) => reports.push(loaded),
  });
  release();
  announceSetupOutcome(document, true);
  expect(reports).toEqual([]);
});

test('the wrapped setup announces success, and failure before rethrowing', async () => {
  const outcomes: boolean[] = [];
  const release = reportPageLoad({
    registeredExtensions: [SUGARCUBES_EXTENSION_NAME],
    document,
    report: (loaded) => outcomes.push(loaded),
  });
  await announcedSetup(document, async () => {})();
  expect(outcomes).toEqual([true]);
  release();
  document.documentElement.removeAttribute(SETUP_OUTCOME_ATTRIBUTE);
  const failure = new Error('setup failed');
  await expect(
    announcedSetup(document, async () => {
      throw failure;
    })(),
  ).rejects.toBe(failure);
  expect(document.documentElement.getAttribute(SETUP_OUTCOME_ATTRIBUTE)).toBe('failed');
});
