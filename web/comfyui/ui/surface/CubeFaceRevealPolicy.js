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
/** Resolve SugarSubstitute-compatible reveal and effective activation state. */
const BYPASS_MODE = 4;
/** Resolve reveal independently from the activation choice it gates. */
export function resolveCubeFaceRevealDecision(node, persisted) {
    const authoredBypass = persisted?.authoredBypass ?? node.mode === BYPASS_MODE;
    const revealable = authoredBypass;
    const revealed = revealable && (persisted?.revealed ?? false);
    const visible = !revealable || revealed;
    const enabledChoice = persisted?.enabledOverride ?? node.mode !== BYPASS_MODE;
    return {
        authoredBypass,
        revealable,
        revealed,
        visible,
        enabledChoice,
        enabled: enabledChoice && visible,
    };
}
