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
/** The orb menu is an ordinary icon menu: its sidebar entries toggle without showing a check. */
import { expect, jest, test } from '@jest/globals';
import { sidebarEntries } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameView.js';

test('ComfyUI’s sidebar tabs are icon entries that toggle and never carry a check', () => {
  const toggle = jest.fn<(id: string) => void>();
  const entries = sidebarEntries({
    tabs: () => [
      { id: 'queue', title: 'Queue', icon: 'pi pi-history' },
      { id: 'node-library', title: 'Node Library', icon: null },
    ],
    toggle,
  });
  expect(entries[0]).toEqual({ label: '', separator: true });
  const tabs = entries.slice(1);
  expect(tabs.map((entry) => entry.label)).toEqual(['Queue', 'Node Library']);
  for (const entry of tabs) {
    expect(entry).not.toHaveProperty('checked');
    expect(entry).not.toHaveProperty('selection');
  }
  tabs[0]!.select?.();
  expect(toggle).toHaveBeenCalledWith('queue');
  expect(sidebarEntries({ tabs: () => [], toggle })).toEqual([]);
});
