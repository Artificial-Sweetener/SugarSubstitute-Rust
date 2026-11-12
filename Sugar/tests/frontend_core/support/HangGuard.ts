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
/**
 * The only elapsed-time bounds tests may use. They end a run that has truly hung; they are not
 * budgets. Tests and gates pass or fail on the outcome they await, never on how long it took, so
 * no test, hook, browser wait or child process carries a timeout of its own.
 */

/** Jest's default for every test and hook, as `testTimeout` in `jest.config.json`. */
export const HANG_GUARD_MS = 600_000;

/**
 * Every Playwright launch, wait and action. It ends a minute before the Jest guard so a hung
 * browser wait fails with its own diagnostics while teardown still runs.
 */
export const OPERATION_HANG_GUARD_MS = HANG_GUARD_MS - 60_000;
