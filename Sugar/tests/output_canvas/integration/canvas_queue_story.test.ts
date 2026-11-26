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
/** Prove each workflow tab's canvas follows the jobs ComfyUI runs, whatever else is queued. */

import { CanvasQueueStory } from '../support/CanvasQueueStory.js';

test('one job at a time shows every preview and final', () => {
  const story = new CanvasQueueStory(['a']);
  story.run(story.queue('a'));
  story.run(story.queue('a'));
  expect(story.violations).toEqual([]);
  expect(story.tabs('a')).toEqual(['base', 'upscale']);
  expect(story.shows('a')).toBe('a2-upscale.png');
});

test('two waiting jobs of one tab each show their previews and finals when they run', () => {
  const story = new CanvasQueueStory(['a', 'b']);
  story.run(story.queue('a'));
  const other = story.queue('b');
  story.start(other);
  const second = story.queue('a');
  const third = story.queue('a');
  story.finish(other);
  story.start(second);
  story.preview(second, 'base');
  story.final(second, 'base');
  story.preview(second, 'upscale');
  story.click('a', 'base');
  story.click('a', 'upscale');
  story.final(second, 'upscale');
  story.finish(second);
  story.run(third);
  expect(story.violations).toEqual([]);
});

test('jobs of two tabs run alternately without touching the other tab', () => {
  const story = new CanvasQueueStory(['a', 'b']);
  const jobs = [story.queue('a'), story.queue('b'), story.queue('a'), story.queue('b')];
  for (const job of jobs) story.run(job);
  expect(story.violations).toEqual([]);
  expect(story.shows('a')).toBe('a2-upscale.png');
  expect(story.shows('b')).toBe('b2-upscale.png');
});

test('three tabs with several waiting jobs each', () => {
  const story = new CanvasQueueStory(['a', 'b', 'c']);
  const jobs: string[] = [];
  for (let round = 0; round < 3; round++)
    for (const tab of ['a', 'b', 'c']) jobs.push(story.queue(tab));
  for (const job of jobs) story.run(job);
  expect(story.violations).toEqual([]);
});

test('a batch whose later acceptances arrive before the first job starts', () => {
  const story = new CanvasQueueStory(['a']);
  const batch = [story.submit('a'), story.submit('a'), story.submit('a')];
  for (const job of batch) story.accept(job);
  for (const job of batch) story.run(job);
  expect(story.violations).toEqual([]);
});

test('a job that starts before its acceptance arrives', () => {
  const story = new CanvasQueueStory(['a']);
  const first = story.submit('a');
  story.start(first);
  story.accept(first);
  story.preview(first, 'base');
  story.final(first, 'base');
  story.preview(first, 'upscale');
  story.final(first, 'upscale');
  story.finish(first);
  expect(story.violations).toEqual([]);
});

test('waiting jobs that ComfyUI runs in another order', () => {
  const story = new CanvasQueueStory(['a']);
  story.run(story.queue('a'));
  const second = story.queue('a');
  const third = story.queue('a');
  story.run(third);
  story.run(second);
  expect(story.violations).toEqual([]);
  expect(story.shows('a')).toBe('a2-upscale.png');
});

test('a waiting job deleted from the queue leaves the tab following the jobs that run', () => {
  const story = new CanvasQueueStory(['a', 'b']);
  story.run(story.queue('a'));
  const other = story.queue('b');
  story.start(other);
  const second = story.queue('a');
  const deleted = story.queue('a');
  story.remove(deleted);
  story.finish(other);
  story.run(second);
  expect(story.violations).toEqual([]);
  expect(story.status('a')).toBe('complete');
  story.run(story.queue('a'));
  expect(story.violations).toEqual([]);
});

test('a run stopped mid-stage keeps its last preview and the next run replaces it', () => {
  const story = new CanvasQueueStory(['a']);
  story.run(story.queue('a'));
  const stopped = story.queue('a');
  const next = story.queue('a');
  story.start(stopped);
  story.preview(stopped, 'base');
  story.final(stopped, 'base');
  story.preview(stopped, 'upscale');
  story.finish(stopped, 'cancelled');
  expect(story.tabs('a')).toEqual(['base', 'upscale']);
  story.run(next);
  expect(story.violations).toEqual([]);
});

test('a run that fails before producing anything leaves the previous results shown', () => {
  const story = new CanvasQueueStory(['a']);
  story.run(story.queue('a'));
  const failed = story.queue('a');
  const next = story.queue('a');
  story.start(failed);
  story.finish(failed, 'error');
  expect(story.shows('a')).toBe('a1-upscale.png');
  story.run(next);
  expect(story.violations).toEqual([]);
});

test('previews resume after the connection drops mid-run', () => {
  const story = new CanvasQueueStory(['a', 'b']);
  const first = story.queue('a');
  story.queue('a');
  story.start(first);
  story.preview(first, 'base');
  story.disconnect();
  story.preview(first, 'base');
  story.final(first, 'base');
  story.preview(first, 'upscale');
  story.final(first, 'upscale');
  story.finish(first);
  expect(story.violations).toEqual([]);
});

test('a click on each listed canvas tab during a run selects it', () => {
  const story = new CanvasQueueStory(['a'], ['base', 'refine', 'upscale']);
  story.run(story.queue('a'));
  const next = story.queue('a');
  story.queue('a');
  story.start(next);
  story.preview(next, 'base');
  story.final(next, 'base');
  story.preview(next, 'refine');
  story.click('a', 'base');
  story.final(next, 'refine');
  story.click('a', 'refine');
  story.preview(next, 'upscale');
  story.click('a', 'upscale');
  story.final(next, 'upscale');
  story.finish(next);
  expect(story.violations).toEqual([]);
});
