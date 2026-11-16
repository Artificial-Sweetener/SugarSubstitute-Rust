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
/** Prove a desktop session failure is titled by the work it left undone. */
import {
  desktopSessionFailureReporter,
  desktopSessionFailureTitle,
} from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionFailureNotice.js';

test('a refused save, a failed restore and a broken handshake are each named for what they are', () => {
  expect(desktopSessionFailureTitle('capture')).toBe('Workspace not saved');
  expect(desktopSessionFailureTitle('materialize')).toBe('Workspace not restored');
  expect(desktopSessionFailureTitle('protocol')).toBe('Desktop session unavailable');
  for (const operation of ['capture', 'materialize', 'protocol'] as const)
    expect(desktopSessionFailureTitle(operation)).not.toContain('canvas');
});

test('the session reporter logs the failure with its operation and shows it under the title of that operation', () => {
  const logged: unknown[][] = [];
  const shown: string[][] = [];
  const report = desktopSessionFailureReporter(
    { error: (...values: unknown[]) => logged.push(values) },
    (severity, title, message) => shown.push([severity, title, message]),
    (error) => (error instanceof Error ? error.message : String(error)),
  );
  const cause = new Error('Desktop checkpoint failed semantic admission: entry submissions.');
  report(cause, 'capture');
  expect(shown).toEqual([
    [
      'error',
      'Workspace not saved',
      'Desktop checkpoint failed semantic admission: entry submissions.',
    ],
  ]);
  expect(logged).toEqual([
    ['SugarCubes desktop session failed', { error: cause, operation: 'capture' }],
  ]);
});
