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
/** Prove external media descriptors cannot introduce traversal or mutable host values into result ownership. */

import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';
import { combineWorkflowMedia } from '../../../frontend/comfyui/ui/execution/domain/WorkflowMedia.js';

test('validates images, video, animation and dimensions while preserving malformed batch holes', () => {
  const decoded = decodeComfyMedia({
    images: [
      { filename: 'still.png', subfolder: 'batch', type: 'temp', width: 3840, height: 2160 },
      null,
      { filename: 'animated.webp' },
    ],
    animated: [false, false, true],
    video: [{ filename: 'clip.mp4', type: 'output' }],
    text: ['literal <b>', 42],
  });
  expect(decoded?.payload.images[0]).toMatchObject({
    file: { filename: 'still.png', subfolder: 'batch', type: 'temp' },
    dimensions: { width: 3840, height: 2160 },
    mediaKind: 'image',
  });
  expect(decoded?.payload.images[1]).toBeNull();
  expect(decoded?.payload.images[2]?.mediaKind).toBe('animated');
  expect(decoded?.payload.videos[0]?.mediaKind).toBe('video');
  expect(decoded?.payload.text).toEqual(['literal <b>']);
});

test.each([
  '../escape.png',
  '/absolute.png',
  'C:\\output.png',
  'https://external/image.png',
  'control\u0000.png',
])('rejects unsafe file reference %s without changing the batch position', (filename) => {
  const decoded = decodeComfyMedia({ images: [{ filename }, { filename: 'valid.png' }] });
  expect(decoded?.payload.images[0]).toBeNull();
  expect(decoded?.payload.images[1]?.file.filename).toBe('valid.png');
});

test('rejects array payloads, unknown file types, invalid dimensions and traversal in subfolders', () => {
  expect(decodeComfyMedia([])).toBeNull();
  const decoded = decodeComfyMedia({
    images: [
      { filename: 'x.png', subfolder: '../outside' },
      { filename: 'x.png', type: 'remote' },
      { filename: 'x.png', width: Infinity, height: 7 },
      { filename: 'animation.gif' },
    ],
  });
  expect(decoded?.payload.images[0]).toBeNull();
  expect(decoded?.payload.images[1]).toBeNull();
  expect(decoded?.payload.images[2]?.dimensions).toBeNull();
  expect(decoded?.payload.images[3]?.mediaKind).toBe('animated');
});

test('copies host records and arrays so later mutation cannot change retained descriptors', () => {
  const image = { filename: 'original.png', width: 100, height: 100 };
  const text = ['original'];
  const decoded = decodeComfyMedia({ images: [image], text });
  image.filename = 'changed.png';
  text.push('changed');
  expect(decoded?.payload.images[0]?.file.filename).toBe('original.png');
  expect(decoded?.payload.text).toEqual(['original']);
  expect(Object.isFrozen(decoded?.payload.images[0]?.file)).toBe(true);
});

test('labels SVG and moving media explicitly instead of mounting them as ordinary image decoders', () => {
  const decoded = decodeComfyMedia({
    images: [{ filename: 'vector.svg' }, { filename: 'clip.webm' }, { filename: 'animation.apng' }],
  });
  expect(decoded?.payload.images.map((entry) => entry?.mediaKind)).toEqual([
    'unsupported',
    'video',
    'animated',
  ]);
});

test('normalizes core, VHS and generic video envelopes without duplicating files', () => {
  const clip = { filename: 'synthetic.webm', type: 'temp', subfolder: 'fixtures' };
  for (const envelope of [{ videos: [clip] }, { gifs: [clip] }, { video: clip }]) {
    expect(decodeComfyMedia(envelope)?.payload.videos[0]).toMatchObject({
      file: clip,
      mediaKind: 'video',
    });
  }
  expect(
    decodeComfyMedia({ images: [clip], videos: [clip], animated: true })?.payload.videos,
  ).toEqual([]);
  expect(
    decodeComfyMedia({ images: [{ filename: 'animated.webp' }], animated: true })?.payload.images[0]
      ?.mediaKind,
  ).toBe('animated');
  expect(
    decodeComfyMedia({ gifs: [{ filename: 'animation.gif' }] })?.payload.videos[0]?.mediaKind,
  ).toBe('animated');
});

test('accepts model file lists and preview tuples while rejecting unsafe and unrelated values', () => {
  expect(
    decodeComfyMedia({ '3d': [{ filename: 'triangle.glb', type: 'output' }] })?.payload.models[0]
      ?.mediaKind,
  ).toBe('model');
  expect(
    decodeComfyMedia({ result: ['models/triangle.glb', null, null] })?.payload.models[0]?.file,
  ).toEqual({ filename: 'triangle.glb', subfolder: 'models', type: 'output' });
  expect(decodeComfyMedia({ result: ['../triangle.glb', null, null] })?.payload.models).toEqual([]);
  expect(decodeComfyMedia({ result: ['some string', null, null] })?.payload.models).toEqual([]);
  expect(
    decodeComfyMedia({ '3d': [{ filename: 'https://external/triangle.glb' }] })?.payload.models,
  ).toEqual([null]);
});

test('replaces restored input previews when a video or model final arrives', () => {
  const initial = decodeComfyMedia({ images: [{ filename: 'input.png', type: 'input' }] })!;
  for (const envelope of [
    { video: { filename: 'clip.webm' } },
    { '3d': [{ filename: 'model.glb' }] },
  ]) {
    const final = decodeComfyMedia(envelope)!;
    expect(combineWorkflowMedia(initial.payload, final.payload, false, false).images).toEqual([]);
  }
});
