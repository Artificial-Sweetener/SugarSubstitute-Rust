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
/** Share the reference 80ms, 24-degree indeterminate motion in every model busy arc. */
export const modelBusyRingCss = `
.sc-model-busy-ring { display:inline-block; flex:none; color:var(--colorBrandStroke1,#4ca6f9); animation:sc-model-busy-spin 1.2s steps(15,end) infinite; }
@keyframes sc-model-busy-spin { to { transform:rotate(360deg); } }
@media(prefers-reduced-motion:reduce) { .sc-model-busy-ring { animation:none; } }
`;
