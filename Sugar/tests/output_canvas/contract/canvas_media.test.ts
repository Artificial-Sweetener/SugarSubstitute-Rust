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
/** Verify exact full-fidelity file URLs and isolated media opening at the public browser boundary. */

import { jest } from '@jest/globals';
import {
  comfyCanvasMediaUrl,
  openComfyCanvasMedia,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasMedia.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

test('encodes literal paths, retains stable revisions, and requests original media without lossy preview flags', () => {
  const base = canvasCatalogFixture().entries[0]!.items[0]!;
  const item = {
    ...base,
    file: { ...base.file, filename: 'a & <b>.png', subfolder: 'nested folder/one' },
  };
  const url = new URL(comfyCanvasMediaUrl('http://localhost:8188/', item));
  expect(url.origin).toBe('http://localhost:8188');
  expect(url.pathname).toBe('/view');
  expect(url.searchParams.get('filename')).toBe('a & <b>.png');
  expect(url.searchParams.get('subfolder')).toBe('nested folder/one');
  expect(url.searchParams.get('revision')).toBe(item.id);
  expect(url.searchParams.has('preview')).toBe(false);
  expect(comfyCanvasMediaUrl(url.origin, item)).toBe(url.href);
  expect(() => comfyCanvasMediaUrl('file:///local', item)).toThrow('unsupported');
});

test('isolates a blank destination before navigating and preserves blocked-popup errors for SugarCubes feedback', () => {
  const item = canvasCatalogFixture().entries[0]!.items[0]!;
  const opened: { opener: unknown; location: { replace: (url: string) => void } } = {
    opener: window,
    location: { replace: jest.fn<(url: string) => void>() },
  };
  const open = jest
    .fn<(url: string, target: string) => typeof opened | null>()
    .mockReturnValue(opened);
  const host = { location: window.location, open };
  openComfyCanvasMedia(host, item);
  expect(open).toHaveBeenCalledWith('about:blank', '_blank');
  expect(opened.opener).toBeNull();
  expect(opened.location.replace).toHaveBeenCalledWith(
    comfyCanvasMediaUrl(window.location.origin, item),
  );
  open.mockReturnValue(null);
  expect(() => openComfyCanvasMedia(host, item)).toThrow('blocked');
});
