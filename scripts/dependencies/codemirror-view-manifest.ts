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
/** One exact literal replacement in both pinned distribution entrypoints. */
export interface CodeMirrorPatchReplacement {
  readonly before: string;
  readonly after: string;
}

/** A previously installed patch generation that installation upgrades in place. */
export interface SupersededCodeMirrorPatch {
  /** Ordered replacements of that generation, reverted in reverse to recover upstream bytes. */
  readonly replacements: readonly CodeMirrorPatchReplacement[];
  /** Installed entrypoint identities of that generation, in manifest entry order. */
  readonly afterSha256: readonly [string, string];
}

/** Reviewed, finite patch identity for CodeMirror's selection synchronization and wrapped viewport. */
export interface CodeMirrorPatchManifest {
  /** Exact package identity, with unchanged upstream MIT license bytes. */
  readonly package: {
    readonly name: string;
    readonly version: string;
    readonly packageSha256: string;
    readonly licenseSha256: string;
  };
  /** Ordered selection, public-focus and wrapped viewport replacements shared by both builds. */
  readonly replacements: readonly CodeMirrorPatchReplacement[];
  /** Earlier generations whose installed bytes are upgraded rather than rejected as drift. */
  readonly superseded: readonly SupersededCodeMirrorPatch[];
  /** Complete entrypoint identities; no other dependency files are changed. */
  readonly entries: readonly [
    {
      readonly path: string;
      readonly beforeSha256: string;
      readonly afterSha256: string;
    },
    {
      readonly path: string;
      readonly beforeSha256: string;
      readonly afterSha256: string;
    },
  ];
}

/** Exact upstream block; preserve indentation and LF bytes. */
const before = `            let track = browser.chrome || browser.ios ? { node: observer.selectionRange.focusNode, written: false } : undefined;
            this.tile.sync(track);
            if (track && (track.written || observer.selectionRange.focusNode != track.node || !this.tile.dom.contains(track.node)))
                this.forceSelection = true;`;

/** Coherent four-endpoint snapshots detect selection changes caused by tile synchronization. */
const after = `            let track = browser.chrome || browser.ios ? { node: observer.selectionRange.focusNode, written: false } : undefined;
            let nativeSelection = track && getSelection(this.view.root), selectionBefore = null;
            if (nativeSelection && observer.selectionRange.eq(nativeSelection)) {
                selectionBefore = new DOMSelectionState;
                selectionBefore.set(nativeSelection.anchorNode, nativeSelection.anchorOffset, nativeSelection.focusNode, nativeSelection.focusOffset);
            }
            this.tile.sync(track);
            let selectionAfter = selectionBefore && getSelection(this.view.root);
            if (track && (track.written || observer.selectionRange.focusNode != track.node || !this.tile.dom.contains(track.node) ||
                selectionAfter && !selectionBefore.eq(selectionAfter)))
                this.forceSelection = true;`;

/** Public focus entry republishes the current CodeMirror selection through its existing forced writer, even when the observer cache is stale. */
const focusBefore = `    focus() {
        this.observer.ignore(() => {
            focusPreventScroll(this.contentDOM);
            this.docView.updateSelection();
        });
    }`;
const focusAfter = `    focus() {
        this.observer.ignore(() => {
            let entering = this.root.activeElement != this.contentDOM;
            focusPreventScroll(this.contentDOM);
            if (entering)
                this.docView.forceSelection = true;
            this.docView.updateSelection();
        });
    }`;

/**
 * Line gaps never start on or inside, or end inside, a replaced range.
 *
 * A gap is itself a replace decoration with the lowest precedence. When it began where a rich
 * widget began (a weight's opening parenthesis at the start of a prompt, say), the widget won the
 * overlap and the gap's spacer was never drawn while its text stayed hidden. The line then measured
 * too short, the next update removed the gap, the one after restored it, and every chip inside was
 * rebuilt several times per keystroke. Gap ends now step outside replaced ranges first.
 */
const gapBoundariesBefore = `                let size = this.gapSize(line, from, to, structure);`;
const gapBoundariesAfter = `                // SugarPrompt: a gap never starts on or inside, or ends inside, a replaced range;
                // overlapping replacements would hide the gap's spacer along with its text.
                for (let moved = true; moved && from < to;) {
                    moved = false;
                    for (let set of this.stateDeco)
                        set.between(from, from, (pointFrom, pointTo, value) => {
                            if (value.point && pointFrom <= from && pointTo > from) {
                                from = pointTo;
                                moved = true;
                            }
                        });
                }
                for (let moved = true; moved && from < to;) {
                    moved = false;
                    for (let set of this.stateDeco)
                        set.between(to, to, (pointFrom, pointTo, value) => {
                            if (value.point && pointFrom < to && pointTo > to) {
                                to = pointFrom;
                                moved = true;
                            }
                        });
                }
                if (to - from < halfMargin)
                    return;
                let size = this.gapSize(line, from, to, structure);`;

/** Wrapped lines that are split keep a 2048-character overscan around the viewport. */
const wrappedMarginBefore =
  'let margin = wrapping ? 10000 /* LG.MarginWrap */ : 2000 /* LG.Margin */';
const wrappedMarginAfter =
  'let margin = wrapping ? 2048 /* SugarPrompt wrapped viewport overscan */ : 2000 /* LG.Margin */';
const displaySizeBefore = 'this.displaySize * (wrapping ? viewState.scaleY : viewState.scaleX)';
const displaySizeAfter = 'this.displaySize / (wrapping ? viewState.scaleY : viewState.scaleX)';
const anchorBefore =
  'scrollAnchorHeight = this.viewState.heightMap.height / this.viewState.scaleY;';
const anchorAfter = 'scrollAnchorHeight = this.viewState.heightMap.height;';

/**
 * Superseded generation's cursor-side replacement, kept only to recover upstream bytes from installs of it.
 *
 * It widened `enforceCursorAssoc`'s one-row overlap test so the browser caret was moved to the row a soft
 * wrap's side names. The editor now draws its own caret, and CodeMirror never enforces the native caret's
 * side while the native selection is hidden.
 */
const enforceAssocBefore = `        if (!before || !after || before.bottom > after.top)
            return;`;
const enforceAssocAfter = `        // SugarPrompt: adjacent row rectangles of a fractionally scaled editor meet within floating point
        // noise; only an overlap of more than half a row means the rectangles share one visual row.
        if (!before || !after || before.bottom > after.top + (after.bottom - after.top) / 2)
            return;`;

/** Seal the upstream 6.43.13 distribution and complete reviewed patched-build identities. */
export const codeMirrorPatchManifest: CodeMirrorPatchManifest = {
  package: {
    name: '@codemirror/view',
    version: '6.43.13',
    packageSha256: 'ebb43f5bc37beb61bb9c0ae0705e8c9b6172bc212b47c1981a43c8cfe2d4782e',
    licenseSha256: '05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07',
  },
  replacements: [
    { before, after },
    { before: focusBefore, after: focusAfter },
    { before: wrappedMarginBefore, after: wrappedMarginAfter },
    { before: gapBoundariesBefore, after: gapBoundariesAfter },
    { before: displaySizeBefore, after: displaySizeAfter },
    { before: anchorBefore, after: anchorAfter },
  ],
  superseded: [
    {
      replacements: [
        { before, after },
        { before: focusBefore, after: focusAfter },
        { before: wrappedMarginBefore, after: wrappedMarginAfter },
        { before: displaySizeBefore, after: displaySizeAfter },
        { before: anchorBefore, after: anchorAfter },
      ],
      afterSha256: [
        '2423dff5df4812fcf03ee551442ca632055d2484da9044e7cc997d89472f661d',
        '9252a026e1150e62404611f38f3f2630276779fc8be0d62dbe669222206649ed',
      ],
    },
    {
      replacements: [
        { before, after },
        { before: focusBefore, after: focusAfter },
        { before: wrappedMarginBefore, after: wrappedMarginAfter },
        { before: gapBoundariesBefore, after: gapBoundariesAfter },
        { before: displaySizeBefore, after: displaySizeAfter },
        { before: anchorBefore, after: anchorAfter },
        { before: enforceAssocBefore, after: enforceAssocAfter },
      ],
      afterSha256: [
        'd5d14844e66b97040fee75eab4e9b48e3cd46f97f13ed9ba2d907a283225df1a',
        '034987a2e1ee927e67d7d689a58a626a0d6869fb961c0b3faa3db98934d1ef04',
      ],
    },
  ],
  entries: [
    {
      path: 'dist/index.js',
      beforeSha256: '5626b4f31cdf55fd6a92be66d28cd40e9446d2caeb10a5b3f3e6c1c02be42950',
      afterSha256: 'b7429658da4c4b981ffc2127cfea34da8b51e1e895e70da6bc0ed5e7b4ec3257',
    },
    {
      path: 'dist/index.cjs',
      beforeSha256: '0ea6fbfd8e5b42ee2926058a929185ac1afffc913a4998e485269f203f7ca17c',
      afterSha256: 'c8940e3b6b1f35af2f24811a9eaeeaf6d8343a0f0531ff8b9e966d5d1a68e343',
    },
  ],
};
