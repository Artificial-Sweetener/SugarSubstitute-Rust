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
/** Translate editor failures and skipped fields into existing structured diagnostics and toasts. */
import type { CubeHostFeedback } from '../../core/CubeHostFeedback.js';
export interface EditorActionLogger {
  error(...values: unknown[]): void;
  warn(...values: unknown[]): void;
}
/** Preserve exception context while keeping routine partial preset application non-blocking. */
export function editorActionFeedback(
  logger: EditorActionLogger,
  feedback: Pick<CubeHostFeedback, 'pushToast' | 'readErrorMessage'>,
): { reportError(error: unknown): void; skipped(names: readonly string[]): void } {
  return {
    reportError: (error) => {
      logger.error('SugarCubes editor action failed', { error });
      feedback.pushToast('error', 'Editor action unavailable', feedback.readErrorMessage(error));
    },
    skipped: (names) =>
      logger.warn('SugarCubes preset skipped incompatible or connected fields', { fields: names }),
  };
}
