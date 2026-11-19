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
/** Prove the page learns dragged file names only from valid host messages for its session. */
import { expect, test } from '@jest/globals';
import { DraggedFiles } from '../../../frontend/comfyui/ui/core/file_drag/DraggedFiles.js';
import { HostFileDragLink } from '../../../frontend/comfyui/ui/core/file_drag/HostFileDragLink.js';

const SESSION = 'session-0123456789abcdef';

function fixture() {
  let receive: ((value: unknown) => void) | null = null;
  const dragged = new DraggedFiles();
  const link = new HostFileDragLink(
    {
      session: SESSION,
      subscribe: (listener) => {
        receive = listener;
        return () => (receive = null);
      },
    },
    dragged,
  );
  link.start();
  const envelope = { protocol: 'sugarcubes.file-drag', version: 1, session: SESSION };
  return { dragged, link, envelope, host: (value: unknown) => receive?.(value) };
}

test('entered names the dragged files in order and left forgets them', () => {
  const f = fixture();
  const heard: (readonly string[] | null)[] = [];
  f.dragged.subscribe(() => heard.push(f.dragged.names));
  f.host({ ...f.envelope, type: 'entered', files: [{ name: 'a.json' }, { name: 'b.png' }] });
  f.host({ ...f.envelope, type: 'left' });
  f.host({ ...f.envelope, type: 'left' });
  expect(heard).toEqual([['a.json', 'b.png'], null]);
});

test('malformed messages and other sessions are ignored', () => {
  const f = fixture();
  f.host({ ...f.envelope, type: 'entered', files: [] });
  f.host({ ...f.envelope, type: 'entered', files: [{ name: 'a.json', path: 'C:/a.json' }] });
  f.host({ ...f.envelope, version: 2, type: 'entered', files: [{ name: 'a.json' }] });
  f.host({
    ...f.envelope,
    session: 'another-0123456789abcdef',
    type: 'entered',
    files: [{ name: 'a.json' }],
  });
  expect(f.dragged.names).toBeNull();
});

test('a retired link forgets the drag it can no longer follow', () => {
  const f = fixture();
  f.host({ ...f.envelope, type: 'entered', files: [{ name: 'a.json' }] });
  f.link.dispose();
  expect(f.dragged.names).toBeNull();
  f.host({ ...f.envelope, type: 'entered', files: [{ name: 'a.json' }] });
  expect(f.dragged.names).toBeNull();
});
