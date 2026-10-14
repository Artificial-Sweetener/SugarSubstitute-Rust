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
/** Safeguard media profile ordering, unchanged observations and lifecycle cleanup. */
import { jest } from '@jest/globals';
import { OutputMediaProfileSource } from '../../../frontend/comfyui/ui/output_saving/OutputMediaProfileSource.js';
import type { OutputMediaProfile } from '../../../frontend/comfyui/ui/output_saving/domain/OutputMedia.js';

test('keeps the newest accepted profile and refuses late responses after disposal', async () => {
  const pending: ((profile: OutputMediaProfile) => void)[] = [];
  const source = new OutputMediaProfileSource(
    () => new Promise((resolve) => pending.push(resolve)),
  );
  const listener = jest.fn();
  source.subscribe(listener);
  const older = source.refresh(),
    newer = source.refresh();
  pending[1]!({ preferredCopyFormat: 'jpeg', fileAction: 'reveal' });
  await newer;
  pending[0]!({ preferredCopyFormat: 'png', fileAction: 'download' });
  await older;
  expect(source.read()).toEqual({ preferredCopyFormat: 'jpeg', fileAction: 'reveal' });
  expect(listener).toHaveBeenCalledTimes(1);
  const last = source.refresh();
  source.dispose();
  pending[2]!({ preferredCopyFormat: 'png', fileAction: 'download' });
  await last;
  expect(listener).toHaveBeenCalledTimes(1);
});

test('does not notify or retain an idle timer when metadata is unchanged', async () => {
  const load = jest
    .fn<() => Promise<OutputMediaProfile>>()
    .mockResolvedValue({ preferredCopyFormat: 'png', fileAction: 'download' });
  const source = new OutputMediaProfileSource(load);
  const listener = jest.fn();
  const stop = source.subscribe(listener);
  await source.refresh();
  await source.refresh();
  expect(listener).not.toHaveBeenCalled();
  stop();
  source.dispose();
  await source.refresh();
  expect(load).toHaveBeenCalledTimes(2);
});
