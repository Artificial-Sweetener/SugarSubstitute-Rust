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

import { describe, expect, jest, test } from '@jest/globals';
import { CubeDefaultReviewService } from '../../frontend/comfyui/ui/save/CubeDefaultReviewService.js';

const request = {
  cubes: [{ cube_id: 'local/me/demo.cube', graph: { nodes: [] } }],
};

function successfulPreview({
  requiresDecision = false,
  overwriteCount = 0,
  promptCount = 0,
  changes = [],
}: {
  requiresDecision?: boolean;
  overwriteCount?: number;
  promptCount?: number;
  changes?: unknown[];
} = {}) {
  return {
    response: { ok: true, statusText: '' },
    data: {
      reviews: [
        {
          cube_id: 'local/me/demo.cube',
          display_name: 'Demo',
          fingerprint: 'stable-fingerprint',
          requires_default_decision: requiresDecision,
          overwrite_default_count: overwriteCount,
          prompt_default_count: promptCount,
          changes,
        },
      ],
    },
  };
}

describe('cube default review service', () => {
  test('attaches the backend fingerprint and aggregate default choices', async () => {
    const api = {
      previewImplementation: jest.fn(async () =>
        successfulPreview({
          requiresDecision: true,
          overwriteCount: 1,
          changes: [
            {
              section: 'Defaults',
              label: 'Height',
              path: 'flavors.authored.default.values.size.height',
              previous_value: 1024,
              proposed_value: 1344,
              previous_exists: true,
              proposed_exists: true,
              decision: 'overwrite_defaults',
            },
          ],
        }),
      ),
    };
    const dialogs = {
      reviewImplementationDefaults: jest.fn(async () => ({
        'local/me/demo.cube': {
          overwriteDefaults: true,
          savePromptFields: false,
        },
      })),
    };
    const service = new CubeDefaultReviewService({ api, dialogs });

    await expect(service.review(request)).resolves.toEqual({
      cubes: [
        expect.objectContaining({
          cube_id: 'local/me/demo.cube',
          default_review: {
            fingerprint: 'stable-fingerprint',
            overwrite_defaults: true,
            save_prompt_fields: false,
          },
        }),
      ],
    });
    expect(dialogs.reviewImplementationDefaults).toHaveBeenCalledTimes(1);
  });

  test('skips the modal when the implementation has no default decision', async () => {
    const api = { previewImplementation: jest.fn(async () => successfulPreview()) };
    const dialogs = { reviewImplementationDefaults: jest.fn(async () => ({})) };
    const service = new CubeDefaultReviewService({ api, dialogs });

    const reviewed = await service.review(request);

    expect(dialogs.reviewImplementationDefaults).not.toHaveBeenCalled();
    expect(reviewed?.cubes).toEqual([
      expect.objectContaining({
        default_review: {
          fingerprint: 'stable-fingerprint',
          overwrite_defaults: false,
          save_prompt_fields: false,
        },
      }),
    ]);
  });

  test('returns null when the author cancels', async () => {
    const api = {
      previewImplementation: jest.fn(async () =>
        successfulPreview({
          requiresDecision: true,
          overwriteCount: 1,
          changes: [
            {
              section: 'Defaults',
              label: 'Height',
              path: 'flavors.authored.default.values.size.height',
              previous_value: 1024,
              proposed_value: 1344,
              previous_exists: true,
              proposed_exists: true,
              decision: 'overwrite_defaults',
            },
          ],
        }),
      ),
    };
    const dialogs = { reviewImplementationDefaults: jest.fn(async () => null) };
    const service = new CubeDefaultReviewService({ api, dialogs });

    await expect(service.review(request)).resolves.toBeNull();
  });

  test('rejects malformed or failed preview responses', async () => {
    const dialogs = { reviewImplementationDefaults: jest.fn(async () => ({})) };
    const failed = new CubeDefaultReviewService({
      api: {
        previewImplementation: jest.fn(async () => ({
          response: { ok: false, statusText: 'Bad Request' },
          data: { error: 'invalid save' },
        })),
      },
      dialogs,
    });
    const malformed = new CubeDefaultReviewService({
      api: {
        previewImplementation: jest.fn(async () => ({
          response: { ok: true, statusText: '' },
          data: { reviews: {} },
        })),
      },
      dialogs,
    });

    await expect(failed.review(request)).rejects.toThrow('invalid save');
    await expect(malformed.review(request)).rejects.toThrow('response is invalid');
  });
});
