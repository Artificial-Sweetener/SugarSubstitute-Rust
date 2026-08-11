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
/** Verify decoded Comfy image lookup across root and nested graph nodes. */

import { ComfyGraphPreviewImageSource } from '../../../frontend/comfyui/ui/surface/ComfyGraphPreviewImageSource.js';

describe('ComfyGraphPreviewImageSource', () => {
  test('finds a nested graph-node image while ignoring cache-busting parameters', () => {
    const image = document.createElement('img');
    image.src = '/view?filename=result.png&type=temp&subfolder=&preview=webp&t=first';
    Object.defineProperties(image, {
      naturalWidth: { value: 1440 },
      naturalHeight: { value: 2016 },
    });
    const rootGraph = {
      _nodes: [
        {
          subgraph: {
            _nodes: [{ imgs: [image] }],
          },
        },
      ],
    };
    const source = new ComfyGraphPreviewImageSource(document, rootGraph);

    expect(source.find('/view?filename=result.png&type=temp&subfolder=&t=second')).toBe(image);
  });

  test('ignores decoded images belonging to another Comfy output', () => {
    const image = document.createElement('img');
    image.src = '/view?filename=other.png&type=temp&subfolder=';
    Object.defineProperties(image, {
      naturalWidth: { value: 512 },
      naturalHeight: { value: 512 },
    });
    const source = new ComfyGraphPreviewImageSource(document, {
      _nodes: [{ imgs: [image] }],
    });

    expect(source.find('/view?filename=expected.png&type=temp&subfolder=')).toBeNull();
  });

  test('uses the source locator when Comfy exposes a transient media URL', () => {
    const image = document.createElement('img');
    image.src = '/view?filename=decoded.png&type=temp&subfolder=';
    Object.defineProperties(image, {
      naturalWidth: { value: 512 },
      naturalHeight: { value: 768 },
    });
    const source = new ComfyGraphPreviewImageSource(document, {
      id: 'root-workflow',
      _nodes: [{ id: 2315, imgs: [image] }],
    });

    expect(source.find('blob:transient-preview', '2315')).toBe(image);
  });
});
