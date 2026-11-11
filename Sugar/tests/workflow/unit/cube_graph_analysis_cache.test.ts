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
/** Prove identical workflow content is analyzed once across runtimes and workflow switches. */
import { expect, jest, test } from '@jest/globals';
import { CubeGraphAnalysisCache } from '../../../frontend/comfyui/ui/workflow/application/CubeGraphAnalysisCache.js';
import type {
  CubeGraphAnalysis,
  CubeGraphGateway,
  CubeGraphSnapshot,
} from '../../../frontend/comfyui/ui/workflow/domain/CubeGraphModels.js';

const analysis: CubeGraphAnalysis = {
  semanticHash: 'a',
  instances: [],
  segments: [],
  placements: [],
};

/** Build a gateway whose analyze requests are recorded by serialized content. */
function recordingGateway() {
  const analyzed: string[] = [];
  const gateway: CubeGraphGateway = {
    analyze: jest.fn(async (snapshot: CubeGraphSnapshot) => {
      analyzed.push(snapshot.serializedWorkflow);
      return analysis;
    }),
    reorder: jest.fn(async () => analysis),
  };
  return { gateway, analyzed };
}

/** Snapshot one workflow with a fresh runtime revision, as each switch does. */
function snapshot(content: string, revision: number): CubeGraphSnapshot {
  return { revision: `root:${String(revision)}:${content}`, serializedWorkflow: content };
}

test('issues no analyze request when switching between unchanged workflows', async () => {
  const { gateway, analyzed } = recordingGateway();
  const cache = new CubeGraphAnalysisCache(gateway);
  const first = JSON.stringify({ nodes: [{ id: 1 }] });
  const second = JSON.stringify({ nodes: [{ id: 2 }] });
  for (let cycle = 0; cycle < 10; cycle++) {
    await cache.analyze(snapshot(first, cycle));
    await cache.analyze(snapshot(second, cycle));
  }
  expect(analyzed).toEqual([first, second]);
});

test('analyzes changed content, retries failures and forgets results on catalog change', async () => {
  const { gateway, analyzed } = recordingGateway();
  const cache = new CubeGraphAnalysisCache(gateway);
  const failing = jest.mocked(gateway.analyze);
  failing.mockRejectedValueOnce(new Error('offline'));
  await expect(cache.analyze(snapshot('a', 0))).rejects.toThrow('offline');
  await cache.analyze(snapshot('a', 1));
  await cache.analyze(snapshot('b', 2));
  await cache.analyze(snapshot('a', 3));
  cache.invalidate();
  await cache.analyze(snapshot('a', 4));
  expect(analyzed).toEqual(['a', 'b', 'a']);
  expect(failing).toHaveBeenCalledTimes(4);
});

test('shares one in-flight request and always submits reorders', async () => {
  const { gateway } = recordingGateway();
  const cache = new CubeGraphAnalysisCache(gateway);
  const [left, right] = await Promise.all([
    cache.analyze(snapshot('a', 0)),
    cache.analyze(snapshot('a', 1)),
  ]);
  expect(left).toBe(right);
  expect(gateway.analyze).toHaveBeenCalledTimes(1);
  await cache.reorder(snapshot('a', 2), ['x'], ['x']);
  await cache.reorder(snapshot('a', 3), ['x'], ['x']);
  expect(gateway.reorder).toHaveBeenCalledTimes(2);
});
