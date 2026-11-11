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
/** Describe the host-addressed HTTP boundary shared by model-library clients. */
export type ModelApiFetch = (path: string, options?: RequestInit) => Promise<Response>;

/** Keep owned API failures accurate while bounding untrusted response text. */
export async function modelApiError(response: Response, fallback: string): Promise<Error> {
  try {
    const payload: unknown = await response.json();
    if (
      typeof payload === 'object' &&
      payload !== null &&
      'error' in payload &&
      typeof payload.error === 'string' &&
      payload.error.length > 0 &&
      payload.error.length <= 500
    )
      return new Error(payload.error);
  } catch (failure: unknown) {
    console.warn('SugarCubes model API returned a non-JSON error response.', failure);
  }
  return new Error(fallback);
}
