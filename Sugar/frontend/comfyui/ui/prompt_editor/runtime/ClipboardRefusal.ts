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
/** Turn a refused clipboard command from the prompt's menu into something the person sees. */
import type { ClipboardObserver } from '../../../../sugarprompt/editor/src/index.js';

/**
 * Observe the prompt's menu clipboard commands for the host.
 *
 * A browser that withholds clipboard access makes the menu's Paste do nothing; Ctrl+V still works
 * because it reads the paste event's own data. A refusal is said aloud instead of passing in
 * silence; every other outcome is the command working as asked. Unexpected failures go to `report`.
 */
export function clipboardRefusalObserver(
  refused: () => void,
  report: (cause: unknown) => void,
): ClipboardObserver {
  return {
    observe: (outcome) => {
      if (outcome.status === 'denied' || outcome.status === 'unavailable') refused();
    },
    error: report,
  };
}
