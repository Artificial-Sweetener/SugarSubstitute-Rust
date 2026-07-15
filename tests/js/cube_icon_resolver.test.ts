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
import { beforeEach, describe, expect, test, jest } from '@jest/globals';
import {
  CubeIconResolver,
  createCubeIconElement,
  deriveDefaultAliasInitials,
  resolveCubeIconModel,
} from '../../web/comfyui/ui/core/CubeIconResolver.js';

describe('cube icon resolver', () => {
  beforeEach(() => {
    HTMLCanvasElement.prototype.getContext = (() => ({
      clearRect: () => {},
      fillText: () => {},
      measureText: () => ({ width: 12 }) as TextMetrics,
      save: () => {},
      restore: () => {},
      scale: () => {},
      translate: () => {},
    })) as unknown as typeof HTMLCanvasElement.prototype.getContext;
  });

  test('derives initials from default alias only', () => {
    expect(deriveDefaultAliasInitials('Text to Image')).toBe('TI');
    expect(deriveDefaultAliasInitials('High-Res Fix')).toBe('HF');
    expect(deriveDefaultAliasInitials('Detailer')).toBe('DE');
    expect(deriveDefaultAliasInitials('3D Pose')).toBe('3P');
  });

  test('drops one complete model prefix when deriving initials', () => {
    expect(deriveDefaultAliasInitials('SDXL/Text to Image')).toBe('TI');
    expect(deriveDefaultAliasInitials('Flux/Image to Image')).toBe('II');
    expect(deriveDefaultAliasInitials('Pony/Promptmask Detailer')).toBe('PD');
  });

  test('keeps boundary slash labels when deriving initials', () => {
    expect(deriveDefaultAliasInitials('/Text to Image')).toBe('TI');
    expect(deriveDefaultAliasInitials('SDXL/')).toBe('SD');
  });

  test('uses fallback text when primary alias has no words', () => {
    expect(deriveDefaultAliasInitials('', { fallbackText: 'SDXL/Text to Image' })).toBe('TI');
    expect(deriveDefaultAliasInitials('', { fallbackText: '' })).toBe('?');
  });

  test('ignores instance alias and group title when resolving placeholders', () => {
    const model = resolveCubeIconModel({
      default_alias: 'Text to Image',
      instance_alias: 'Hero Prompt Builder',
      title: 'Draft Name',
    });

    expect(model.kind).toBe('initials');
    expect(model.initials).toBe('TI');
  });

  test('asset descriptor wins over placeholder initials', () => {
    const model = resolveCubeIconModel({
      cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
      default_alias: 'Text to Image',
      icon: {
        kind: 'asset',
        url: '/sugarcubes/assets/icon?cube_id=demo',
        media_type: 'image/png',
      },
    });

    expect(model.kind).toBe('asset');
    if (model.kind !== 'asset') throw new Error('Expected asset icon');
    expect(model.url).toBe('/sugarcubes/assets/icon?cube_id=demo');
    expect(model.initials).toBe('TI');
    expect(model.fallback).toEqual({
      fontFamily: 'Segoe UI',
      fontWeight: 700,
      inset: 2,
      renderSize: 96,
    });
  });

  test('browser asset icon element keeps image rendering path', () => {
    const element = createCubeIconElement(document, {
      default_alias: 'Text to Image',
      icon: {
        kind: 'asset',
        url: '/sugarcubes/assets/icon?cube_id=demo',
        media_type: 'image/png',
      },
    });

    expect(element.querySelector('img')?.getAttribute('src')).toBe(
      '/sugarcubes/assets/icon?cube_id=demo',
    );
    expect(element.querySelector('canvas')).toBeNull();
  });

  test('asset path descriptor builds the backend icon URL from cube id', () => {
    const model = resolveCubeIconModel({
      cube_id: 'Artificial-Sweetener/Base-Cubes/Text to Image.cube',
      default_alias: 'Text to Image',
      icon: {
        kind: 'asset',
        path: 'assets/icons/Text to Image.png',
        media_type: 'image/png',
      },
    });

    expect(model.kind).toBe('asset');
    if (model.kind !== 'asset') throw new Error('Expected asset icon');
    expect(model.url).toBe(
      '/sugarcubes/assets/icon?cube_id=Artificial-Sweetener%2FBase-Cubes%2FText%20to%20Image.cube',
    );
  });

  test('empty default alias falls back to cube id initials', () => {
    const model = resolveCubeIconModel({ cube_id: 'demo' });

    expect(model.kind).toBe('initials');
    expect(model.initials).toBe('DE');
    expect(model.fallback).toEqual({
      fontFamily: 'Segoe UI',
      fontWeight: 700,
      inset: 2,
      renderSize: 96,
    });
  });

  test('image load failure remains available for initials fallback', () => {
    let imageRef: HTMLImageElement | null = null;
    const resolver = new CubeIconResolver({
      imageFactory: () => {
        imageRef = document.createElement('img');
        return imageRef;
      },
      onImageLoad: jest.fn(),
    });
    const model = resolver.resolve({
      default_alias: 'Text to Image',
      icon: {
        kind: 'asset',
        url: '/icon.png',
      },
    });

    const loading = resolver.getImage(model);
    expect(loading.status).toBe('loading');
    imageRef!.dispatchEvent(new Event('error'));
    expect(resolver.getImage(model).status).toBe('error');
    expect(model.initials).toBe('TI');
  });

  test('browser icon element renders markup-like aliases literally', () => {
    const element = createCubeIconElement(document, {
      default_alias: '<img src=x>',
    });

    expect(element.dataset.initials).toBe('IS');
    expect(element.textContent).toBe('');
    expect(element.querySelector('img')).toBeNull();
    expect(element.querySelector('canvas.sugarcubes-cube-icon__fallback')).not.toBeNull();
    expect(element.getAttribute('style')).toBeNull();
  });

  test('browser fallback icon element uses prefixed alias body initials', () => {
    const element = createCubeIconElement(document, {
      default_alias: 'SDXL/Text to Image',
    });
    const canvas = element.querySelector<HTMLCanvasElement>(
      'canvas.sugarcubes-cube-icon__fallback',
    );

    expect(element.dataset.initials).toBe('TI');
    expect(element.classList.contains('is-initials')).toBe(true);
    expect(canvas).not.toBeNull();
    expect(canvas!.width).toBe(96);
    expect(canvas!.height).toBe(96);
  });
});
