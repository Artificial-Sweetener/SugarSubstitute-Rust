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
  readonly replacements: readonly [
    CodeMirrorPatchReplacement,
    CodeMirrorPatchReplacement,
    CodeMirrorPatchReplacement,
    CodeMirrorPatchReplacement,
    CodeMirrorPatchReplacement,
  ];
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
    {
      before: 'let margin = wrapping ? 10000 /* LG.MarginWrap */ : 2000 /* LG.Margin */',
      after:
        'let margin = wrapping ? 2048 /* SugarPrompt wrapped viewport overscan */ : 2000 /* LG.Margin */',
    },
    {
      before: 'this.displaySize * (wrapping ? viewState.scaleY : viewState.scaleX)',
      after: 'this.displaySize / (wrapping ? viewState.scaleY : viewState.scaleX)',
    },
    {
      before: 'scrollAnchorHeight = this.viewState.heightMap.height / this.viewState.scaleY;',
      after: 'scrollAnchorHeight = this.viewState.heightMap.height;',
    },
  ],
  entries: [
    {
      path: 'dist/index.js',
      beforeSha256: '5626b4f31cdf55fd6a92be66d28cd40e9446d2caeb10a5b3f3e6c1c02be42950',
      afterSha256: '2423dff5df4812fcf03ee551442ca632055d2484da9044e7cc997d89472f661d',
    },
    {
      path: 'dist/index.cjs',
      beforeSha256: '0ea6fbfd8e5b42ee2926058a929185ac1afffc913a4998e485269f203f7ca17c',
      afterSha256: '9252a026e1150e62404611f38f3f2630276779fc8be0d62dbe669222206649ed',
    },
  ],
};
