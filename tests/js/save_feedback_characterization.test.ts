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
import { describe, expect, test } from '@jest/globals';
import {
  SugarCubeExportError,
  buildErrorDetail,
  formatEndpoint,
  formatSaveSummaryEntry,
  formatViolations,
} from '../../web/comfyui/ui/save/SaveFeedback.js';

describe('save feedback characterization', () => {
  test('export error normalization preserves structured backend fields', () => {
    const original = new SugarCubeExportError('Original', 'detail', [{ id: 1 }]);

    expect(SugarCubeExportError.from(original)).toBe(original);
    expect(SugarCubeExportError.from(null)).toMatchObject({ message: 'Export failed' });
    expect(
      SugarCubeExportError.from({
        message: 'Backend rejected save',
        detail: 'Invalid graph',
        violations: [{ from: { id: 1 }, to: { id: 2 } }],
      }),
    ).toMatchObject({
      message: 'Backend rejected save',
      detail: 'Invalid graph',
      violations: [{ from: { id: 1 }, to: { id: 2 } }],
    });
    expect(SugarCubeExportError.from('offline')).toMatchObject({ message: 'offline' });
  });

  test('save summaries distinguish committed, uncommitted, and failed-commit artifacts', () => {
    expect(
      formatSaveSummaryEntry({
        committed: true,
        default_alias: 'Demo',
        path: 'demo.cube',
        commit_short_sha: 'abc1234',
        commit_message: 'save Demo',
      }),
    ).toBe('saved and committed: Demo -> demo.cube (abc1234: save Demo)');
    expect(
      formatSaveSummaryEntry({
        committed: true,
        default_alias: 'Demo',
        path: 'demo.cube',
        commit_short_sha: 'abc1234',
      }),
    ).toBe('saved and committed: Demo -> demo.cube (abc1234: committed)');
    expect(
      formatSaveSummaryEntry({
        committed: false,
        default_alias: 'Demo',
        path: 'demo.cube',
        commit_error: 'working tree locked',
      }),
    ).toBe('saved only: Demo -> demo.cube (commit failed: working tree locked)');
    expect(formatSaveSummaryEntry({})).toBe('saved only: SugarCube -> ');
  });

  test('error details prefer readable strings and serialize structured details', () => {
    expect(buildErrorDetail({ detail: 'Readable failure', details: { ignored: true } })).toBe(
      'Readable failure',
    );
    expect(buildErrorDetail({ details: { node: 'KSampler', field: 'cfg' } })).toBe(
      '{"node":"KSampler","field":"cfg"}',
    );
    expect(buildErrorDetail(null)).toBe('');
    expect(buildErrorDetail({ details: 'not structured' })).toBe('');

    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(buildErrorDetail({ details: circular })).toBe('');
  });

  test('violation formatting preserves endpoint labels and missing-field fallbacks', () => {
    const violations = [
      {
        from: { title: 'Source', cube: 'cube-a', port: 0 },
        to: { title: 'Target', cube: 'cube-b', port: 'image' },
      },
      { from: { id: 12 }, to: null },
    ];

    expect(formatViolations(violations)).toBe(
      'Source [cube-a] (0) -> Target [cube-b] (image)\n12 -> <unknown>',
    );
    expect(formatViolations([])).toBe('');
    expect(formatViolations(null)).toBe('');
    expect(formatEndpoint({ title: '', cube: '', id: 0 })).toBe('<unknown>');
    expect(formatEndpoint('invalid')).toBe('<unknown>');
  });
});
