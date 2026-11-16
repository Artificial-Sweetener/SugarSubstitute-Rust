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
/** Word a desktop session failure by what it left undone, never by an unrelated surface. */
import type { DesktopSessionFailedOperation } from '../domain/DesktopSession.js';

/** Title the notice for one failed desktop session operation. */
export function desktopSessionFailureTitle(operation: DesktopSessionFailedOperation): string {
  if (operation === 'capture') return 'Workspace not saved';
  if (operation === 'materialize') return 'Workspace not restored';
  return 'Desktop session unavailable';
}

/**
 * Build the reporter the desktop session uses: each failure is logged with the work it
 * interrupted and shown under that work's own title.
 */
export function desktopSessionFailureReporter(
  logger: Pick<Console, 'error'>,
  notify: (severity: 'error', title: string, message: string) => void,
  describe: (error: unknown) => string,
): (error: unknown, operation: DesktopSessionFailedOperation) => void {
  return (error, operation) => {
    logger.error('SugarCubes desktop session failed', { error, operation });
    notify('error', desktopSessionFailureTitle(operation), describe(error));
  };
}
