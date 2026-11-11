//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
/** Verify permission-checked transitions into the Cube definition editor. */

import { CubeEditorEntryController } from '../../../frontend/comfyui/ui/surface/CubeEditorEntryController.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { jest } from '@jest/globals';

const node = { id: 17 } as CubeNode;

describe('CubeEditorEntryController', () => {
  test('leaves the surface before opening an authorized Cube definition', async () => {
    const calls: string[] = [];
    const controller = new CubeEditorEntryController({
      canEdit: async () => true,
      leaveSurface: () => calls.push('leave'),
      recoverSurface: () => calls.push('recover'),
      openEditor: () => calls.push('open'),
      onDenied: () => calls.push('denied'),
      logger: console,
    });

    controller.open(node);
    await Promise.resolve();

    expect(calls).toEqual(['leave', 'open']);
  });

  test('keeps the surface mounted when definition access is denied', async () => {
    const leaveSurface = jest.fn();
    const openEditor = jest.fn();
    const onDenied = jest.fn();
    const controller = new CubeEditorEntryController({
      canEdit: async () => false,
      leaveSurface,
      recoverSurface: jest.fn(),
      openEditor,
      onDenied,
      logger: console,
    });

    controller.open(node);
    await Promise.resolve();

    expect(onDenied).toHaveBeenCalledWith(node);
    expect(leaveSurface).not.toHaveBeenCalled();
    expect(openEditor).not.toHaveBeenCalled();
  });

  test('recovers the surface and preserves failure context when entry fails', async () => {
    const error = new Error('navigation failed');
    const recoverSurface = jest.fn();
    const logger = { error: jest.fn() };
    const controller = new CubeEditorEntryController({
      canEdit: async () => true,
      leaveSurface: jest.fn(),
      recoverSurface,
      openEditor: () => {
        throw error;
      },
      onDenied: jest.fn(),
      logger,
    });

    controller.open(node);
    await Promise.resolve();

    expect(recoverSurface).toHaveBeenCalledTimes(1);
    expect(logger.error).toHaveBeenCalledWith('SugarCubes could not open the Cube editor.', {
      cubeNodeId: node.id,
      error,
    });
  });
});
