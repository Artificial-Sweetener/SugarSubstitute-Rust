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
/** One managed service admission owner separates public retirement from actual storage settlement. */
export class ManagedWildcardStorageAdmission {
  private requests = 0;
  private mutation = false;

  /** Latest read/metadata work may overlap; only an admitted mutation excludes new storage requests. */
  get mutationActive(): boolean {
    return this.mutation;
  }

  /** Persistence requires every actual storage request, including retired reads, to have settled. */
  get mutationBlocked(): boolean {
    return this.mutation || this.requests > 0;
  }

  /** Reserve mutation ownership synchronously before latest-lane retirement can invoke host reentry. */
  beginMutation(): boolean {
    if (this.mutationBlocked) return false;
    this.mutation = true;
    return true;
  }

  /** Release the reservation only after actual mutation work and its cache/metadata handoff settle. */
  endMutation(): void {
    this.mutation = false;
  }

  /** Count actual storage work before callbacks/host dispatch; retirement never releases this ownership. */
  async run<T>(work: () => Promise<T>): Promise<T> {
    this.requests++;
    try {
      return await work();
    } finally {
      this.requests--;
    }
  }
}
