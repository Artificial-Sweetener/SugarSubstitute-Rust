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
/** Expose native Mica through the negotiated frame without changing browser presentation. */
export const desktopFrameCss = `
  [data-sc-desktop-frame-surface] { background:transparent !important; }
  [data-sc-desktop-frame-host] { border:0 !important; box-shadow:none !important; }
  [data-sc-desktop-frame-host] .sc-substitute-frame { app-region:drag; }
  [data-sc-desktop-frame-host] .sc-substitute-frame :is(
    button,input,select,textarea,a,[role='tab'],[role='tablist'],[data-sc-button],
    .sc-workflow-strip,.sc-generation-strip,.sc-app-orb,.sc-orb-actions
  ) { app-region:no-drag; }
`;
