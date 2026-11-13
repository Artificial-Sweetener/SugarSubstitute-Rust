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
/** Prove content switching preserves native controls and retires hidden or late media. */
import { jest } from '@jest/globals';
import { BrowserCanvasMediaDetail } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaDetail.js';
import { BrowserCanvasMediaSessions } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasMediaSessions.js';
import {
  CanvasModelRuntimeFixture,
  mixedMediaFixture,
} from '../support/CanvasMixedMediaFixture.js';
import { settleCanvasImages } from '../support/CanvasImageFixture.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

/** Mount production presentation while replacing only the external model runtime. */
function fixture() {
  const media = mixedMediaFixture();
  const root = document.createElement('div');
  document.body.append(root);
  const models = new CanvasModelRuntimeFixture();
  const sessions = new BrowserCanvasMediaSessions();
  const clock = new WorkflowTestClock();
  const options = {
    models,
    sessions,
    clock,
    url: (item: { file: { filename: string } }) =>
      `http://localhost/view?filename=${item.file.filename}`,
    changed: jest.fn(),
    report: jest.fn(),
  };
  const detail = new BrowserCanvasMediaDetail(root, options);
  return { ...media, root, models, sessions, clock, options, detail };
}

beforeEach(() => {
  jest.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
  jest.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {});
});
afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('uses native video controls, retains identity on refresh, and restores paused inspection after overview', () => {
  const f = fixture();
  f.detail.reconcile(f.plan(1), true);
  const video = f.root.querySelector('video')!;
  expect(video.controls).toBe(true);
  expect(video.autoplay).toBe(false);
  video.dispatchEvent(new Event('loadeddata'));
  expect(f.detail.status()?.state).toBe('ready');
  video.currentTime = 1.25;
  f.detail.reconcile(f.plan(1), true);
  expect(f.root.querySelector('video')).toBe(video);
  f.detail.reconcile(f.plan(1, true), true);
  expect(video.pause).toHaveBeenCalled();
  expect(video.hasAttribute('src')).toBe(false);
  expect(f.detail.diagnostics()).toEqual({ videos: 0, models: 0 });
  f.detail.reconcile(f.plan(1), true);
  f.detail.reconcile(f.plan(0), true);
  expect(f.sessions.video(f.items[1]!.id)?.time).toBe(1.25);
  f.detail.reconcile(f.plan(1), true);
  const restored = f.root.querySelector('video')!;
  Object.defineProperty(restored, 'duration', { value: 3 });
  restored.dispatchEvent(new Event('loadedmetadata'));
  expect(restored.currentTime).toBe(1.25);
  f.detail.dispose();
});

test('rejects a retired model completion and switches image, model, video and animation without retained viewers', async () => {
  const f = fixture();
  f.detail.reconcile(f.plan(2), true);
  expect(f.detail.status()?.state).toBe('loading');
  f.detail.reconcile(f.plan(1), true);
  expect(f.models.requests[0]?.signal.aborted).toBe(true);
  const late = f.models.viewer();
  f.models.requests[0]!.resolve(late);
  await settleCanvasImages();
  expect(late.dispose).toHaveBeenCalledTimes(1);
  expect(f.root.querySelector('video')).not.toBeNull();
  f.detail.reconcile(f.plan(2), true);
  const current = f.models.viewer();
  f.models.requests[1]!.resolve(current);
  await settleCanvasImages();
  expect(f.detail.status()?.state).toBe('ready');
  f.detail.reconcile(f.plan(0), true);
  expect(current.dispose).toHaveBeenCalledTimes(1);
  expect(f.detail.element.hidden).toBe(true);
  f.detail.reconcile(f.plan(3), true);
  const animation = f.root.querySelector('img')!;
  expect(animation.alt).toBe('Animated output');
  f.detail.reconcile(f.plan(3), false);
  expect(animation.hasAttribute('src')).toBe(false);
  expect(f.detail.element.hidden).toBe(true);
  f.detail.dispose();
});

test('exposes a failed host viewer for retry and never opens a browser dialog', async () => {
  const f = fixture();
  const alert = jest.spyOn(window, 'alert');
  f.detail.reconcile(f.plan(2), true);
  f.models.requests[0]!.reject(new Error('Synthetic model failure'));
  await settleCanvasImages();
  expect(f.detail.status()).toMatchObject({ state: 'error', canRetry: true });
  f.detail.retry(f.plan(2), true);
  expect(f.models.requests).toHaveLength(2);
  expect(alert).not.toHaveBeenCalled();
  f.detail.dispose();
  const late = f.models.viewer();
  f.models.requests[1]!.resolve(late);
  await settleCanvasImages();
  expect(late.dispose).toHaveBeenCalledTimes(1);
});

test('bounds a stalled host load and rejects its late completion after timeout', async () => {
  const f = fixture();
  f.detail.reconcile(f.plan(2), true);
  expect(f.clock.pending).toBe(1);
  f.clock.advance(15001);
  expect(f.detail.status()).toMatchObject({ state: 'error', canRetry: true });
  expect(f.models.requests[0]?.signal.aborted).toBe(true);
  const late = f.models.viewer();
  f.models.requests[0]!.resolve(late);
  await settleCanvasImages();
  expect(late.dispose).toHaveBeenCalledTimes(1);
  expect(f.clock.pending).toBe(0);
  f.detail.dispose();
});
