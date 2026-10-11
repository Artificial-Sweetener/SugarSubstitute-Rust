# SugarSubstitute chronological work ledger

## Outcome and scope

Checkpoint K1 delivers SugarSubstitute: the native splash starts promptly, an existing ComfyUI installation starts or is explicitly attached, and its actual frontend with installed SugarCubes appears in a native frameless main window. Windows uses Mica where supported and a themed fallback otherwise. K1 includes governed execution, supervision, multi-instance resource ownership, crash capture, watchdog behavior and bounded shutdown.

After K1, integrate SugarCubes host awareness, canvas tabs and static native display of real Comfy image outputs. Windows, macOS and Linux are supported throughout; Linux native qualification covers X11 and Wayland.

This milestone excludes existing-install adoption or cleanup, legacy settings import, session translation/restoration, installers/updaters, ComfyUI environment provisioning, real CuteCanvas integration, editing/layers/history, zoom/pan, binary live previews and application GPU-selection controls. ComfyUI retains its existing Python environment. Rust desktop and splash use no Qt runtime. Native rasterization, image preparation and scheduling prioritize CPU rendering; the browser retains its normal acceleration defaults.

## Completed implementation before this ledger

These components already exist. Continue from them; their outstanding acceptance checks appear in S0 and the later integration stages.

| ID | Completed implementation | Source | Remaining acceptance or integration |
| --- | --- | --- | --- |
| B1 | Rust workspace, pinned Rust 1.93.1/edition 2024, locked dependencies and executable composition | Cargo.toml, Cargo.lock, rust-toolchain.toml, crates/sugarsubstitute | Production launch composition is K1. The current executable runs the splash demonstration. |
| B2 | Architecture, testing, persistence, cache and vendoring policies/checkers; repository verification commands and three-platform CI configuration | governance/, tools/xtask/, .github/workflows/verify.yml | Extend registrations/checks with each implemented owner. Configured CI is not native platform acceptance. |
| B3 | Execution ownership restrictions and checker fixtures | governance/execution/policy.toml, tools/xtask/src/execution*.rs | M0 has registered the tested execution adapter; native application integration and qualification remain pending. |
| B4 | CPU splash renderer, paper-flip animation, progress/activity drawing, blank console presentation and collapsed/expanded interaction | crates/sugarsubstitute-splash/, crates/sugarsubstitute/src/demo.rs | S0 verifies full reference parity, control behavior and responsiveness. L2 supplies the live transcript. |
| B5 | Forty packaged character poses, eighteen caption glyph PNGs and asset provenance | crates/sugarsubstitute-splash/assets/, third_party/ | Preserve assets and licensing; S0 checks all poses, scales and phases. |
| B6 | Five-locale product registry and localized demonstration controls | crates/sugarsubstitute-contracts/src/localization.rs | M0 owns shared localization; L2 completes production messages. |
| B7 | Replaceable content/frame contract, Tao event loop, native drag/close and platform presenters: Windows DIB/Mica, macOS softbuffer and Linux GTK/Cairo | crates/sugarsubstitute-desktop/ | S0 qualifies the demonstration. L1 adds the resizable main window, Wry and production native-window behavior. |
| B8 | Deterministic Rust render export, Python reference renders, comparison metrics and Windows native observation tools | crates/sugarsubstitute/src/capture.rs, tools/splash_parity/ | S0 adds enforceable parity decisions and completes native/timing receipts; L4 extends lifecycle fixtures. |

Current presentation rasterizes and validates full frames synchronously. S0 uses visible-state invalidation and a coalesced sixty-hertz animation cadence; static, hidden and minimized surfaces wait for meaningful events. M0 provides governed off-thread preparation before production integration.

**Baseline verification receipt (2026-10-11, integration lead):** The existing B1–B8 sources passed `cargo xtask verify` on Windows with pinned `1.93.1-x86_64-pc-windows-msvc`: governance, formatting, strict workspace/all-target/all-feature Clippy, 62 unit tests (host 3, desktop 5, splash 25, tooling 29), and warning-free workspace Rustdoc. Cargo.lock SHA-256: `a723f0b1dd6fadea5e43489ad9165dbf70a62bd3b406fa67df0944e881042ccb`. This proves the portable/Windows owner tests for the baseline; S0 native parity/input/timing and all macOS/Linux execution remain pending. The maintainer authorized interval commits; preserve this initial baseline before S0 worktrees. Worktrees use `E:\devprojects\` exclusively.

The source baseline uses tiny-skia 0.12.0, cosmic-text 0.19.0, png 0.18.1 and Tao 0.37.0. Native acceptance of the current splash remains pending for Windows, macOS and Linux X11/Wayland. Existing rendering/export tools are implemented; complete 1:1 parity, external visible/interactive startup timing and smooth-drag acceptance remain outstanding.

## Reference owners and integration boundaries

- The original SugarSubstitute repository is the behavioral reference for splash, lifecycle, leases, locks and crash reporting. Build idiomatic Rust owners and adapters from those contracts.
- ComfyUI owns workflow graph/documents/tabs and inference. Its server provides /extensions, /view, /history/{prompt_id}, WebSocket and startup arguments.
- SugarCubes owns editor/panels/menus/toolbars, desktop styling, canvas tab controls and web layout intent inside the actual frontend. K1 uses the installed extension unchanged. Its frontend/comfyui/ui.ts registers SugarCubes.UI; WEB_DIRECTORY points to web and scripts/build-web.ts produces served artifacts.
- SugarSubstitute owns application lifecycle, connection/process services, native windows, OS integration, native workspace document inventory and workflow/run/result coordination.
- CuteCanvas owns future editing/rendering. Its headless Editor and separate QPane presentation provide adapter references; this milestone implements a replaceable static mock.
- SugarPrompt remains integrated through SugarCubes.

Canvas documents and Comfy workflow documents have separate owners. SugarCubes projects native workspace snapshots and sends activation/close intents; Rust acknowledges authoritative state. The splash observes startup events and emits intents. Application services own startup policy. Runtime-independent domain/contracts remain free of Tao/Wry/GTK/AppKit/Win32, fonts, HTTP clients and borrowed native handles.

Existing SugarCubes queue/result owners are ComfyPromptQueueBridge, CubeHostExecutionComposition, the direct execution client, ComfyCubeOutputEventBridge, the history adapter and CubeOutputExecutionStore. sugarcubes/runtime/cube_output_images.py emits filename/subfolder/type descriptors. C0/P4 trace both queue paths; a latest-value Cube store alone does not identify every workflow/prompt result.

The inspected integration baseline is SugarCubes 0.15.0 and served Comfy frontend 1.47.11 with /scripts compatibility shims. The separate frontend checkout declaring 1.30.3 is a source reference. Match implementation to actual served identities and supported APIs. Reference base revisions are SugarCubes c6fedb6c2776d4ab847aac45ba2d91c2aa9289fd, CuteCanvas f8fdedb6eede4bdfefae8244c4a2c71b398195b1, Comfy server a1c421994cdcc5044dbce2bb7628e89386311cc5 and separate frontend 95b3b509c700b84cd50672751a13a2e0612a6ff0. Implementation receipts also identify relevant working-tree and served artifacts.

The legacy reference is the production SplashWindow, SplashFeedback, SplashProgressPanel, paper-flip/pose owners and transcript: 558 × 558 logical geometry, forty poses and en, zh-Hans, ja, ko and es.

## Work status and order

Implementation and verification have separate statuses. Implementation is Not started, Partial or Complete. Verification is Pending, Passing or Failing, recorded separately for Windows, macOS and Linux X11/Wayland. An existing component stays completed while its outstanding checks remain explicit. Accept a stage only when its required checks pass for the stated artifacts/platforms.

| Order | Work ID | Implementation | Verification | Next work and prerequisite |
| --- | --- | --- | --- | --- |
| 1 | S0 | Complete on the Windows development setup: controls, raster parity, clipping and bounded source audit | Windows 11 at 96 DPI accepted: 429 frames, five startup runs, native controls/drag/load/idle matrix pass; other platforms and mixed DPI pending | Proceed to M0; retain separately pending native platform/DPI qualification. |
| 2 | M0 | In progress: registered execution adapter, configuration/readiness contracts, immutable frames and publication admission implemented | Windows workspace verification passes 211 tests, including 43 execution tests; native integration/performance pending | Integrate governed raster preparation, nonblocking native delivery and startup composition. |
| 3 | M1 | Not started | Pending | Implement supervision, scoped identities/leases, authenticated lifecycle and containment after M0. |
| 4 | L1–L5 | Not started: reusable presentation/proof components are B4–B8 | Pending | Build main host, production splash, Comfy adapter, qualification and native crash capture in parallel after M0/M1 ports are accepted. |
| 5 | K1 | Not started | Pending | Integrate and hand off the first actual Comfy/SugarCubes host after L1–L5 components are ready. |
| 6 | C0 | Not started | Pending | Implement desktop wire/provenance/layout contracts from the working K1 host. |
| 7 | P1–P6 | Not started | Pending | Build native composition, negotiation, SugarCubes desktop UI, outputs, static mock and harness in parallel after C0. |
| 8 | I1 | Not started | Pending | Integrate empty native canvas after P1 feasibility and P2/P3/P5/P6 readiness. Output imports are not prerequisites. |
| 9 | I2 | Not started | Pending | Integrate actual image outputs and acknowledged tabs after I1 and P4/P5/P6 readiness. |
| 10 | Q1 | Not started | Pending | Qualify the full application after I2; preserve and extend earlier platform receipts. |
| 11 | H1 | Not started | Pending | Deliver reproducible developer artifacts and actual launch commands after Q1. |

### Execution and parallel assignments

The numbered stages give implementation order. Entry conditions apply to dependent work on its receiving platform; independent fixture preparation, source checks and other-platform qualification can proceed concurrently. Accepting one platform does not accept unrun platforms.

Use Codex native subagents for parallel lanes. Before dispatch, record current dirty files, relevant source/lockfile/served artifact identities and running previews. Use the existing checkouts with exclusive file ownership; a default-HEAD worktree must not omit necessary uncommitted work. Preserve unrelated work and build outputs.

The integration lead owns manifests/lockfile, shared exports, composition, policy/catalog registrations, generated contracts, CI and ledger updates. Each worker receives exact paths, a frozen boundary, required evidence and a reviewer. Shared-file changes go through the lead and update all affected consumers. Coordinate Cargo resource use. Native display/focus/timing scenarios run one harness-owned window at a time; isolated portable tests run concurrently.

Keep the development Comfy root, trusted interpreter, arguments, profile selection and artifact paths in ignored local configuration or invocation arguments. Launch and attach are explicit modes. K1 uses the existing installation and installed extension without changing their source/builds.

Record acceptance beside the relevant work item: implementation status; verification result/platform; source/lockfile/served/runtime identity; actual command/scenario and intended test count; observed result; unresolved requirement and next action. Store bulky captures/traces and their temporary files in permitted ignored storage. PLAN.md owns implementation order/status; AGENTS.md and executable governance retain engineering policy.

## 1. S0 — Qualify the existing splash and finish its remaining gaps

**Entry:** B4–B8 are implemented. **Owner:** Native presentation lead. **Deliverable:** Acceptance of the existing blank-console splash demonstration, with parity, native input/material and external startup/responsiveness receipts.

**Existing implementation:** Renderer, animation, progress/activity, packaged assets, disclosure controls, native presentation and comparison/export tools are present.

**Remaining work:** Resolve full-frame reference differences, add a passing/failing parity decision, complete control/accessibility/platform qualification, reduce unnecessary redraw work and measure visible/interactive startup.

### 1.1 S0.1 — Preserve the existing demonstration boundary and verify reference behavior

**Implementation:** Complete for the existing demonstration (B4–B8). **Verification:** Pending.

- [ ] Verify the existing demonstration against the production reference; preserve assets, behavior and the presentation boundary.

Continue from the implemented sugarsubstitute-splash and sugarsubstitute-desktop owners. Keep the scripted clock/progress source in the demonstration/qualification driver; it is not application startup. S0 neither launches ComfyUI nor creates Wry, desktop IPC or canvas contracts. Preserve the replaceable content/prepared-frame/presenter boundary and one native event loop.

Use the real Python/PySide splash through tools/splash_parity/reference.py, Rust deterministic exports through the implemented --scenarios/--output options and tools/splash_parity/compare.py for side-by-side/difference evidence. Python/Qt is a qualification reference only. Preserve all forty 386 × 386 character PNGs, guest credits and provenance, caption icon licenses, logical centering, expanded/collapsed geometry and color/accent behavior. Register any new derivatives with their source/license in third_party/manifest.toml and keep assets in their consuming owner.

Paper-flip behavior retains the observed affine mapping, minimum edge width, shear, vertical bob and landing settle. Defaults are 950 ms hold with 150 ms jitter, 390 ms flip, numbered weight 1, guest weight 0.25, married.png weight 0.125 and existing recent-pose penalties. Inject timestamps/seeds; test forward/backward flips, edge phases, recency and reduced motion. Match current progress geometry and distinguish completion from activity.

Keep tiny-skia/cosmic-text local to presentation. They provide graphics/text primitives; they do not justify building a general widget framework. softbuffer 0.4.8's public pixel format is opaque RGB and cannot itself provide the required transparent splash. Windows proves native alpha/backdrop composition; macOS/Linux prove their platform presentation and themed fallbacks. The parity requirement remains 1:1 with the platform's Python reference; report each difference rather than excusing it as a renderer change. [CPU graphics](https://github.com/linebender/tiny-skia), [text shaping](https://github.com/pop-os/cosmic-text), [buffer format](https://docs.rs/softbuffer/0.4.8/softbuffer/struct.Buffer.html).

### 1.2 S0.2 — Finish outstanding presentation and proof work in parallel

**Implementation:** Partial; implementation/proof tools exist, remaining lane checks pending. **Verification:** Pending.

- [ ] Complete the outstanding graphics/native-control/accessibility checks and fix their failures through the assigned owners.

| Parallel assignment | Exclusive ownership | Required output |
| --- | --- | --- |
| S0-G graphics | Existing splash animation/projection/character/progress/render modules and their tests | Deterministic alpha, clipping, sampling, theme and scale comparisons for all poses/phases |
| S0-N native presentation | Desktop runtime/platform presenters and native-lifetime/input tests | Real window, composition, hover/click/disclosure/close/drag/focus and DPI proof |
| S0-Q qualification | tools/splash_parity and controlled fixtures; no runtime-owner edits | Production reference, side-by-side/difference review and native scenario receipts |
| Lead integration | Executable driver, shared resources/exports/manifests/catalogs | One reproducible demo; owned text complete in the sole locale registry |

S0 keeps the console blank as requested. Production transcript parsing, selection, scrolling and full accessible styled output are L2 tasks after M0. S0 still proves accessible identities for existing controls, keyboard disclosure/close behavior, reduced-motion behavior and locale text. This sequencing preserves the demonstration scope while making execution available before expensive production text/render work.

### 1.3 S0.3 — Close parity, redraw and startup qualification gaps

**Implementation:** Partial; enforceable parity, native controls and redraw fixes implemented. **Verification:** Graphics-owner parity passes; combined current-artifact native/source-audit/timing receipts pending.

- [ ] Add enforceable parity decisions, correct redundant redraws and qualify displayed startup, interaction and dragging.

**In-progress receipt (2026-10-11, integration lead):** Baseline `6d9222a` is committed after the B1–B8 Windows gate receipt above. S0-G, S0-N and S0-Q use exclusive worktrees under `E:\devprojects\`; lead owns composition, manifests and this ledger. Initial graphics changes add same-owner bootstrap rendering, monotonic frame timestamps and mascot widget clipping. `cargo test --locked -p sugarsubstitute-splash` selected 29 tests, all passed on Windows; focused strict Clippy passed in the graphics worktree. The lead's `cargo test --locked -p sugarsubstitute capture::tests` selected one passing test for explicit locale/control-state qualification inputs; `cargo build --locked -p sugarsubstitute` passed. These are development receipts, not S0 acceptance.

Fresh production-reference qualification now exercises all forty poses, forward/backward phases, dark/light expanded/collapsed views, caption hover/press states and scales 1/1.25/1.5/1.75/2. Receipt identifiers are `s0/reference-20261011-01`, `s0/reference-20261011-02`, `s0/candidate-20261011-01` and `s0/baseline-identities.local.json` in ignored permitted artifact storage. The first comparison exposed a half-pixel canvas-size rounding disagreement at scale 1.75; the reference now uses the same nearest-pixel rounding as the native-size exporter. Reference 01 is superseded for that scale. Full current comparison, reviewed parity decisions, native controls/accessibility/drag/DPI and compositor startup/interaction distributions remain outstanding. MacOS and Linux X11/Wayland qualification remain pending. Next actions: integrate reviewed native input/redraw changes, rerun current paired renders and native qualification, resolve every failing owner region, then run exact-content repository gates before the next coherent commit. M0/M1 production implementation waits for the Windows S0 boundary; independent contract and capture-lifetime investigation is proceeding.

**Windows parity failure receipt (2026-10-11):** The completed initial candidate export contains 379 PNGs; its process exit was observed. `python tools/splash_parity/compare.py --reference <reference-20261011-02> --candidate <candidate-20261011-01> --output <comparison-20261011-01>` rejected all 379 scenarios under explicit alpha-aware decisions; failing owners are character, close, console, frame, progress and status. Each receipt records reference/candidate PNG SHA-256 and owner metrics, with paired images, overlays and both composited/premultiplied differences. This failure is retained while fractional clipping/text fixes are measured in a smaller current subset. The integrated `python -m unittest tools.splash_parity.test_acceptance tools.splash_parity.test_native_decisions` ran 13 passing tests, including missing controls, wrong pose/animation, hidden alpha changes and fractional dimensions; test output/temporary storage uses the permitted artifact drive. Passing comparator tests do not establish production parity. Next actions: fix the observed regions, compare the exact rebuilt candidate and run the native matrix before advancing S0.

**Integrated Windows owner receipt (2026-10-11):** `cargo xtask verify` from the primary checkout passed governance, formatting, strict Clippy, 79 tests (host 7, desktop 10, splash 33, tooling 29) and warning-free Rustdoc after rebuilding the four authored packages. Four hidden-HWND tests exercise real subclass registration, removal, failed removal/registration and reentrant final destruction. The embedded application manifest activates common-controls v6; callback references remain owned until confirmed removal or final native destruction. The integrated Python acceptance/observation suite ran 16 passing tests. Each worktree now uses its own target directory: sharing artifacts had reused an `xtask` executable with another checkout embedded, so earlier shared-target gate output is not primary-checkout acceptance. Commit `688f766` rejects that mismatch explicitly; its exact standalone contents passed `cargo xtask verify` with 63 tests in an isolated checkout, and launching that checkout's verifier from the primary checkout was observed to fail before governance. Subsequent manifest/text changes require a fresh exact-content gate.

**Initial native observation (2026-10-11):** `cargo build --release --locked -p sugarsubstitute` produced the executable recorded as `s0/native-build-01`, SHA-256 `f84b9adc4d8540ef0b5b11d51840045daff1d1459672f8af1c2a25f3591a2939`. The sequential owned-window harness observed compositor character paint at 431.66 ms and a causal keyboard disclosure change at 461.04 ms. This single run meets the 500/750 ms targets; it is not a cold/warm distribution or uncontended qualification. The initial managed UI Automation observer failed to initialize standard Win32 proxies, exposing buttons as panes. An independent unmodified standard-button fixture reproduced the observer failure. The corrected observer explicitly initializes those proxies in a managed method; the product uses standard pushbuttons with CPU paint suppression. The failing receipts `s0/native-dark-en-01`, `s0/native-dark-en-02` and the independent observer fixture remain preserved.

**Native controls and idle receipt (2026-10-11):** Exact release `s0/native-build-02`, SHA-256 `450f8a1e83929e6ba35d839fe27d3b73de794761cc3523812a49d54c04c2d2cc`, passed `native_windows.py --input --accessibility --compositor` in `s0/native-dark-en-06`: Mica, unoccluded compositor paint, Tab disclosure, native Button identities/Invoke/focus, Space disclosure, pressed close feedback, cancelled close restoration and accessible cancellation before the demonstration deadline. Three independent `operational_windows.py --mode cpu` receipts (`s0/native-static-{visible,minimized,hidden}-01`) observed unchanged process CPU ticks over each 30-second interval, below 1% of one logical core, and actual repaint after restore. These qualify only the S0 demonstration process. `--mode dpi` observed one real 96-DPI monitor; mixed-monitor DPI remains unavailable and pending. The light reduced-motion scenario exposed a posted-pointer observation failure after passing compositor/disclosure/accessibility; real pointer qualification is being added. The unchanged native view owner's focused regression proves light pressed/cancelled pixels while animation remains stopped. These receipts precede the newest graphics/text changes and require current-artifact requalification.

**Windows text receipt (2026-10-11):** The integrated native DirectWrite owner retains bounded synchronous shaping callbacks, owned font faces and grayscale coverage. `cargo test --locked -p sugarsubstitute-splash` ran 36 passing tests, including native fallback, invalid input preserving pixels and repeated font-owner teardown. `s0/text-native-locales-candidate-03/status-parity-receipt.json` records all fifty raw status regions byte-exact against production Windows references: five locales, five scales and both themes. Native callback/raster bounds and scoped FFI ownership were reviewed; portable text remains independently implemented and unexecuted on macOS/Linux. The tooling suite now runs 36 passing tests, including delayed compositor paint, explicit rejection of missing/late/occluded/foreign captures, actual mouse ownership and cleanup. Full-frame graphics parity and the complete current native/timing matrix remain pending.

**Actual pointer and locale receipt (2026-10-11):** `s0/native-light-en-reduced-03` passed real SendInput close press/cancel, neutral restoration and accessible close with the exact build-02 artifact. This resolves the posted-pointer harness failure while retaining its failed receipts. `s0/native-dark-{zh-Hans,ja,ko,es}-01` passed the same native input/accessibility/compositor scenario with explicitly supplied registry labels, supplementing the English dark receipt. The root focused regression `cargo test --locked -p sugarsubstitute demo::tests::static_light_close_press_and_cancel_repaint_without_animation` selected one passing test. Receiving-platform acceptance still requires the final integrated artifact, full parity and measured timing/dragging.

**Control/text interval gate (2026-10-11, integration lead):** Commit `0307809` delivers the integrated native-control, static-redraw, bootstrap and Windows-text contents. Exact contents passed `cargo xtask verify` in the primary checkout: governance, formatting, strict Clippy, 84 tests (host 8, desktop 10, splash 36, tooling 30) and warning-free Rustdoc on pinned Windows MSVC Rust 1.93.1. The Python owner suite ran 37 passing tests after common startup observation switched to real SendInput for both products. `git diff --check` passed. These checks do not advance S0 or any later stage to accepted.

**Graphics development receipt (2026-10-11):** The completed release export `s0/graphics-full-14` contains all 379 frames with observed successful exporter exit. Console regions passed all 209 applicable cases and disclosure regions passed all 379; six character cases, one progress case and sixteen fractional close-feedback cases still failed unchanged decisions. The subsequent graphics-owner export `s0/graphics-full-15` passes every progress, console and caption region. Its trial character-coordinate change regressed other poses and was reverted; only the four independently passing border/progress/chrome source files were integrated. The primary splash suite now selects 45 passing tests, including native text, allocator-cause preservation and fractional caption coverage. Full current combined export remains pending. No comparator threshold or owner mask was weakened.

**Current native development receipt (2026-10-11):** `s0/native-build-03` records the committed control/text release, executable SHA-256 `1ce4aac36a099189885d9a35965ffcafdc538ee1243266c5438081fe80aa6c92`. Its fifty locale glyph regions are byte-exact; the larger status owner still exposed fractional progress clipping before the subsequent graphics integration. Release `s0/native-build-04`, SHA-256 `51a4da6f8cf42c33af4f14fce8ca65f8a7c01195d6e7118552b0753694b1c850`, records the integrated graphics source fingerprints and retained PDB. `s0/native-build04-dark-en-01` passed native input/accessibility/compositor qualification with mean synchronous frame work 2.31 ms. `s0/native-build04-static-visible-01` observed unchanged CPU ticks over 30.00 seconds, stable 13 GDI objects and actual restore paint. `s0/native-build04-input-latency-01` measured 32 real caption gestures through unoccluded compositor pixels: p50 50.16 ms, p95 86.60 ms and p99 91.31 ms, including capture sampling cost; concurrent graphics work prevents an uncontended-distribution claim. Physical drag receipt `s0/native-build04-static-drag-01` failed to enter the OS move loop. Inspection found the pinned Tao drag adapter posts a stack pointer where Windows expects packed cursor coordinates; the native owner is correcting that boundary.

**Startup ownership development receipt (2026-10-11):** The tooling suite selects 60 passing tests. Reference startup now retains the trusted venv launcher and verifies its direct base-interpreter child by executable, creation time, liveness and retained native handle. Stdlib-only smoke receipts prove both reference-child and direct contention-worker ownership and terminal cleanup. `s0/startup-reference-owner-probe-03` admitted the actual 558-by-558 Qt window but failed the initial foreground/unoccluded boundary. The reference observer is being aligned with the production launcher's `CREATE_NO_WINDOW` behavior before repeating timing. No reference activation is injected and no timer is restarted. The earlier Rust probe meets targets but overlaps compilation and is not uncontended acceptance. Controlled-load, current physical-drag and paired startup distributions remain pending.

**Reference correction (2026-10-11):** Qt's offscreen backend used FreeType and only the manually loaded Segoe UI family; the native Windows backend uses the installed Windows font environment. The earlier offscreen text evidence is superseded for Windows status parity, including CJK fallback. Fresh production renders use the Windows backend and `WA_DontShowOnScreen` with the actual show/layout lifecycle, preserving correct caption geometry without mapping a visible window. `s0/reference-windows-20261011-01` contains 379 frames; current `s0/reference-windows-{en,zh-Hans,ja,ko,es}-03` contain ten locale samples each. Environment receipts preserve the Qt platform/version, font inventory and legacy source hashes. Complete frame parity remains pending.

Compare the actual Python and Rust renders side by side at identical clock, pose, dimensions, theme, accent, font environment and scale. Review every scenario and each owner region; report pixel differences and overlay them. Extend existing harness coverage for fractional/high DPI, controls, all poses, flip edges and native fallback. The comparator enforces alpha-aware, per-owner expectations and rejection fixtures covering missing controls and wrong animation. Preserve those regression decisions while resolving remaining graphics failures. Both automated evidence and visual inspection are required.

Measure external process-start to visible first frame, Rust-entry to first submission, and first interactive control separately. The existing DesktopReceipt measures submissions, not compositor visibility. Declare cold/warm runs precisely; do not purge user/browser caches to simulate cold startup. Store bulky captures/traces and their temporary files in permitted local artifact storage, with only portable references in this ledger.

Initial development targets are visible first frame within 500 ms and interactive controls within 750 ms on the documented qualification setup; compare Python and Rust in the same setup. These are acceptance targets, not measured claims. Record distributions and first-run font-discovery costs. Bootstrap one packaged pose before full font discovery/all-pose decoding; enrichment must preserve the same view owner.

Use dirty-state/hover-change invalidation and coalesced redraw requests. Pointer movement that does not change visible state must not trigger full rasterization. Keep animation at at most the existing approximately 60 Hz cadence, skip missed periods and avoid catch-up loops. Static, reduced-motion, hidden and minimized surfaces should sleep until meaningful events. Check smooth dragging under idle and controlled CPU contention before assigning blame to the executor. S0 has no background scheduler; any offloaded production preparation waits for M0.

**Acceptance:** The blank-console demonstration works through the production presentation boundary; parity differences are resolved and recorded; close/disclosure/animation/progress/native material/input and timing scenarios pass. Record separate Windows, macOS and Linux X11/Wayland receipts. Unavailable platform receipts remain pending, and independent source/fixture work can proceed; no unrun platform is claimed accepted.

## 2. M0 — Build and enforce application execution before integration lanes

**Entry:** The reusable S0 presentation boundary and demonstration are accepted on the platform used for development; other platform qualification can run independently. **Owner:** Execution lead; governance reviewed independently. **Deliverable:** One supervised execution adapter and minimal application/startup contracts.

### 2.1 M0.1 — Introduce the actual layered owners

**Implementation:** Complete for first implemented concerns: contracts, application, execution and Comfy configuration owners are registered. **Verification:** Windows workspace verification passes; native application integration pending.

- [x] Implement the real application/contracts/execution/Comfy owners as their first behavior lands; register each implemented crate.

Keep sugarsubstitute as executable/composition owner. Introduce sugarsubstitute-contracts for runtime-independent typed boundaries and sugarsubstitute-application for application state/use cases when their first real behavior lands. Introduce sugarsubstitute-execution as the dedicated execution adapter and sugarsubstitute-comfy for process/readiness transport. Register each real crate with architecture/testing policy and nonzero behavioral tests in the same change. No empty placeholder crates, identity-only proofs, raw native handles in domain state or tooling dependencies in runtime crates.

The host injects typed execution ports into application services and owns the executor's lifetime. Application owns decisions/scopes/generations; execution owns runtime construction, threads/task spawning, I/O scheduling, CPU admission, deadlines, task supervision, terminal outcomes and worker shutdown. M1's application supervision owner controls process/resource lifetimes, external crash observation and restart authorization through these ports. Task supervision and process supervision have distinct authorities. Desktop owns main-thread event pumping, native operations and delivery of acknowledged current-generation results. No consumer gets a runtime handle, raw spawn API, detached task or scheduler escape hatch.

### 2.2 M0.2 — Fix a bounded runtime policy

**Implementation:** Complete for bounded execution admission, supervised job lifetimes and owned worker shutdown. **Verification:** 43 execution tests and the full Windows workspace gate pass; receiving native performance and process/network integration remain pending.

- [x] Implement bounded runtime/worker admission, job lifetimes, cancellation and completion delivery.

Use one private Tokio 1.53.2 runtime per process that performs background work for async process/network/timer work, its capped blocking facility for finite infrastructure operations, and a separate fixed execution-owned CPU worker facility for render/decode preparation. Published metadata reports MSRV 1.71; pin required reviewed features and verify the complete lockfile on Rust 1.93.1. Do not import Tokio into consumers. Every executable mode uses the same governed execution adapter; a separate supervisor process does not authorize consumer-created runtimes or threads. Aggregate supervisor/desktop/helper costs are included in Q1 measurements. Native event-loop callbacks are not background jobs; GLib/AppKit/Win32 scheduler entry points must not become alternative executors.

Start with two async runtime threads, two capped runtime blocking workers for finite filesystem/infrastructure work, and a separate fixed CPU facility with at most two workers, reducing CPU workers to one on a single-logical-CPU host. Only the execution adapter creates/joins these workers and owns their bounded queue. With two CPU workers, reserve one for interactive frame preparation and one for background image decode; with one, chunk/cooperatively yield background work and prioritise interactive jobs. Configure every cap explicitly; do not accept the runtime's all-core async/512-blocking defaults.

Permit eight ordinary concurrent I/O jobs plus four reserved lifecycle/process-drain/readiness slots, and thirty-two queued admitted jobs including four reserved lifecycle slots. Persistent pipe readers use async I/O, not occupied blocking workers. No image job can consume the reserved lifecycle or runtime-infrastructure capacity. Bound infrastructure admission as well: Tokio filesystem/DNS use its blocking facility, so async capacity alone does not reserve their execution. Validate saturation with active decode/raster/finite filesystem work and maintain process/cancellation responsiveness.

Admit before spawning, including memory reservations. Allow one image decode and at most one replaceable splash/static-frame preparation per surface; keep only its newest pending revision. Configuration is reviewed at this owner, not selected by individual workers or scaled to every available CPU by default.

Tokio's blocking facility does not provide the required application queue/resource policy. Started blocking work cannot generally be aborted; runtime timeout shutdown is not proof that it stopped. Require cooperative cancellation between bounded chunks, boundary timeouts and observed completion. Use an owned helper process only for genuinely required operations that cannot otherwise be bounded/interrupted, with explicit process ownership; never silently detach them. [Pinned runtime policy](https://docs.rs/tokio/1.53.2/tokio/runtime/struct.Builder.html), [blocking-task lifecycle](https://docs.rs/tokio/1.53.2/tokio/task/fn.spawn_blocking.html).

Define scope ID, job ID, operation kind, connection/navigation/import generation, deadline, capacity/memory reservation, cancellation state and typed terminal result. Model success, failure, panic, cancellation, timeout and overload distinctly. Panics are observed failures and do not escape into the event loop. Dropping a future/token invalidates publication but does not prove blocking work ceased. Scopes cancel children, drain joins, release reservations and record unresolved shutdown failures.

A bounded completion mailbox wakes Tao once per pending batch. Control/failure/cancellation notifications have reserved capacity; replaceable status/frame snapshots coalesce. Output acknowledgements and terminal outcomes cannot be silently dropped. Apply a bounded per-event-loop drain budget, re-wake remaining work, validate generations before publication and avoid broad locks held across awaits or native callbacks.

**Execution owner integration receipt (2026-10-11):** `sugarsubstitute-execution` is the registered sole scheduling owner. Its private Tokio runtime uses two async and at most two blocking workers; its CPU pool uses two workers, or one on a single-logical-CPU machine. Admission bounds twelve concurrent I/O jobs with four lifecycle reserves, two infrastructure jobs with one lifecycle reserve, thirty-two ready factories with four lifecycle reserves, forty-eight retained terminal/cleanup credits with four lifecycle reserves, sixty-four live/retained scopes, and 256 MiB of admitted retained storage. Ready-factory capacity and deferred disposal capacity are distinct. Replaced factories, abandoned results and cancellation callbacks retain their credits and byte reservations until worker-owned cleanup actually returns. The owner supervises panic payloads, custom cancellation wakers and their destructors, and observes joins before runtime destruction. Supplied clock/control/callback code runs outside admission locks. Transferred frame/cache guards may outlive stopped workers while retaining accountable byte charges.

Independent source review accepted all twenty-seven worker-owned files against `D:\\SugarSubstitute-Rust\\artifacts\\m0\\execution-retirement-20261011-01.json`, SHA-256 `5e894485c9ff93e13f2fc24cec8ee37ae1bb81cada7fd226b0831297d6642c10`. Canonical `cargo check --offline -p sugarsubstitute-execution` deliberately added Tokio 1.53.2, tokio-macros 2.7.2 and the workspace package edge; existing external lockfile versions were preserved. `cargo xtask verify` passed governance, formatting, strict all-target/all-feature Clippy, 211 Rust tests (43 execution), and warning-free Rustdoc with pinned Rust 1.93.1 Windows MSVC. These tests cover production-boundary admission, reserves, aggregate memory, actual cancellation, timeout/panic, replacement, disposal, bounded delivery and worker teardown. The native demonstration still uses its synchronous raster path until the governed presentation integration lands. This receipt does not establish macOS/Linux execution or native M0 latency.

### 2.3 M0.3 — Prepare production rendering and observability

**Implementation:** Partial: immutable prepared frames, incremental validation, aggregate allocation claims and application publication admission are implemented. Governed raster delivery and native instrumentation remain pending. **Verification:** Windows contracts 29 and application 11 tests pass; native M0 performance is not established.

- [ ] Integrate governed render preparation, retained frames, bounded diagnostics and latency/resource instrumentation.

Move expensive font discovery, decoding, layout/raster preparation and image validation off the UI thread through this owner after the bootstrap frame. Publish immutable validated prepared frames with explicit pixel format, dimensions, stride, scale, generation and bounded buffer leases. Validate pixels once at preparation, then check identity/shape at native submission rather than rescanning every pixel on the UI thread. Reuse admitted buffers and retain the last frame for expose/restore; release them through one owner. Platform-specific copies/blits remain measured thread-affine presentation work.

Instrument queue wait, runtime execution, UI delivery, raster preparation, frame submission, dropped/coalesced updates, allocated/reserved bytes, cancellation lag and scope shutdown. Use structured logging keyed by application/connection/run/document/job IDs. Keep bounded log storage and redact tokens, credentials and unnecessary paths. Do not route unlimited tracing output into the splash or use runtime printing.

Initial responsiveness targets: p95 host UI callback/drain work below 4 ms and p99 below 8 ms, p95 input-to-feedback below 50 ms, and p95 submitted splash intervals below 25 ms in the controlled 60 Hz scenario. Also measure displayed frames separately. Static native canvas must have no periodic redraw loop; hidden/minimized surfaces must stop preparation. Targets are not hardcoded test sleeps or evidence of existing performance. Retain the measured setup locally and report host/browser/Comfy CPU and memory separately.

**Prepared-frame boundary receipt (2026-10-11):** The contracts owner validates dimensions, tight RGBA stride, finite scale, actual vector capacity and premultiplication in steps of at most 65536 pixels. Validation failure remains sticky; incomplete or failed validation cannot publish. Each distinct frame allocation obtains a non-cloneable byte claim from the already-admitted shared lease. Atomic claims prevent separate vectors from reusing the same bytes; prepared-frame clones share one allocation and claim, and final storage release precedes claim reuse. The application publication owner accepts only the current surface/state/shape, visible state and a sequence newer than its last accepted sample, allowing already-running animation samples to publish while newer samples are requested. Resize/visibility revisions advance before mutation; no-op and generation-exhaustion paths preserve state. Qualification reviewed both the original boundary and aggregate-charge correction. Delivery identities and exact hashes are in `m0/presentation-contracts-20261011-01.json` under permitted artifact storage. Primary-checkout `cargo xtask verify` passed governance, formatting, strict Clippy, all 155 Rust tests and warning-free Rustdoc on Rust 1.93.1 Windows MSVC. This verifies the reusable boundary; the demonstration still prepares frames synchronously until the governed raster/native integration lands. Other platforms and native M0 latency/resource qualification remain pending.

### 2.4 M0.4 — Prove behavior and enforce scheduling ownership

**Implementation:** Scheduler registration, owner-export AST checks and rejection fixtures are implemented and independently reviewed. **Verification:** All 37 tooling tests, 43 execution tests and governance pass on Windows; receiving native qualification pending.

- [x] Register the implemented scheduler and extend the existing execution gate; prove runtime behavior and rejection cases.

Extend governance/execution/policy.toml with exactly one implemented scheduler package. Keep raw runtime/private worker dependencies inside it. Reject scheduler access elsewhere, including tests/platform conditionals, renamed dependencies/imports, re-exports, aliases, macro fragments and native FFI worker entry points. Extend rejection fixtures for actual new APIs; audit raw identifiers and false-positive handling rather than weakening the gate. Static checking cannot establish lifecycle safety, so pair it with behavior tests.

Required tests cover admission saturation/byte reservations, fairness between classes, coalescing, cancellation before queue/admission/during execution, timeout, panic/error reporting, current-generation publication, bounded completion delivery, no UI wait, repeated scope teardown, producer flood, and shutdown with queued/running I/O and CPU work. Tests use deterministic clocks and explicit synchronization through production execution contracts. Native qualification proves event-loop responsiveness and thread affinity. Observe joined workers/children and released memory; never declare clean shutdown merely because a deadline expired.

### 2.5 M0.5 — Freeze only first-checkpoint contracts

**Implementation:** Partial: shared localization, bounded readiness admission and validated local startup configuration are implemented. Process/readiness transport and actual startup composition remain pending. **Verification:** Seven readiness protocol tests, five readiness admission tests and thirteen configuration tests pass on Windows; actual frontend probing remains L1/L3 integration.

- [ ] Implement and freeze minimal startup/readiness/configuration/localization ports for the first host.

Freeze startup events, splash intents, process/readiness ports and the minimal versioned readiness receipt. K1 receipt is at most 4 KiB, one accepted completion per current navigation generation, carrying launch/navigation identity, observed frontend/extension evidence and typed failure. It exposes no desktop commands. Register its implemented cross-version boundary in persistence governance.

Define ignored startup.local.toml/CLI inputs: launch or attach, trusted root/interpreter/argument array or loopback HTTP root, explicit port, new-app locale/theme/reduced-motion selection and isolated profile identity. Use safe platform paths and atomic, validated writes for any new app-owned settings/profile metadata; catalog schemas and preserve original bytes on failure. This is new configuration, not migration. Consolidate product messages under one explicit localization owner and the existing five-locale registry; application must not depend on splash solely to obtain text.

**Local configuration boundary receipt (2026-10-11):** `sugarsubstitute-comfy` now owns parsing borrowed startup-document bytes, with architecture/testing registration and schema-one persistence catalog identity. The parser rejects documents over 64 KiB before UTF-8/TOML parsing, unknown or duplicate fields, unsupported versions and missing/mixed connection tables. The domain validates explicit launch/attach choice, absolute lexical root/interpreter paths, nonzero matching port, bounded Unicode argument arrays, isolated profile identity and exact supported locale. Attachment accepts only root HTTP origins at `127.0.0.1`, `[::1]` or `localhost`; credentials, queries, fragments and other paths/host spellings are rejected. Theme has one domain owner with the supported splash export retained. Outer diagnostics redact supplied data while preserving original error causes. All fields are explicit: `schema_version`, `mode`, `port`, `profile`, `locale`, `theme`, `reduced_motion` and exactly one `[launch]` (`root`, `interpreter`, `arguments`) or `[attach]` (`origin`) table. Parsing does not establish filesystem trust or process ownership and performs no settings writes. Independent source review accepted the boundary. Deliberate offline lock resolution added only the new workspace package; all external dependency versions remain unchanged. Primary-checkout `cargo xtask verify` passed governance, formatting, strict Clippy, all 168 Rust tests and warning-free Rustdoc on Rust 1.93.1 Windows MSVC. Governed file loading, CLI startup composition and canonical filesystem/resource trust validation remain integration work; other platforms are unqualified.

The lead freezes M1's application supervision, instance/resource ownership, native containment and authenticated lifecycle protocol paths first. M1 registers actual adapter packages when their behavior lands. Freeze launch lane paths against its accepted ports: L1 desktop window/frame/webview/bootstrap_probe; L2 splash rendering/transcript/native splash-view; L3 Comfy startup/readiness/config adapters; L4 qualification tooling/fixtures; L5 native crash capture/incident/reporting adapters. Route shared native events through L1's desktop authority; process ownership remains with M1.

**Acceptance:** Execution lifecycle, admission/performance instrumentation, deterministic tests and governance rejection cases pass; layered launch ports compile; actual scheduler ownership is registered; no raw scheduler escapes exist. M1 implementation starts after this foundation is accepted; L1-L5 dependent integration starts after M1 acceptance. Independent source/fixture preparation and other-platform qualification may proceed without claiming dependent acceptance. Generated TypeScript, desktop handshake and canvas contracts remain later work.

## 3. M1 — Establish application supervision and resource ownership

**Entry:** M0 execution/contracts accepted on the receiving platform; independent fixture/research and other-platform work can proceed. **Owner:** Application supervision lead and native lifecycle reviewers. **Deliverable:** Small external supervisor, resource-specific coordination, authenticated lifecycle protocol and qualified containment, exercised with controlled desktop/backend fixtures before real Comfy integration.

### 3.1 M1.1 — Establish the process arrangement and lifetime authority

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement executable supervisor/desktop/guardian modes and their explicit lifetime authorities.

Use the SugarSubstitute executable in explicit internal supervisor, desktop and POSIX-guardian modes. A launch elects or connects to the relevant resource coordinator; each requested independent desktop receives a distinct instance/run identity and supervised lifetime. One supervisor coordinates multiple desktops and managed backend resources within its scoped namespace. Separate namespaces can coexist. It exits after its clients/resources settle; this milestone installs no background service.

The desktop owns the splash, native event loop, window/webview and eventual canvas presentation. The supervisor owns launch admission, client/backend leases, process identities, application exit classification, shutdown policy and authorized restart. It uses the M0 execution adapter, while application services remain the authority for policy. Locks, leases, connection observations and UI-progress observations are cohesive components within that supervisor, not additional watchdog processes. Browser subprocesses retain engine-owned lifetimes; never stop them by executable-name searches or broad unrelated-child cleanup.

L5 supplies one independent native crash-capture handler and a reporting mode that can operate when the hosted frontend is unavailable. Windows kernel containment needs no extra guardian. Linux/macOS add one guardian per supervisor when required to clean its owned process groups after supervisor loss. Handler lifetime is independent of the failed desktop's cleanup group. Register modes, allowed process creation/native scheduling boundaries and narrow FFI exceptions with governance when implemented; all background scheduling still passes through the execution owner.

The splash receives immutable startup events and emits typed intents. A desktop channel closing, an authorized GUI/webview reload and terminal application shutdown are separate events. Supervisor loss makes dependent desktops fail closed and invokes native containment; desktop loss does not invalidate other desktops' leases. Helpers have explicit endpoint/handle ownership and last-client/last-resource exit rules, so supervision does not become a source of orphan processes.

### 3.2 M1.2 — Freeze identities, leases, locks and authenticated lifecycle messages

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement scoped resource identities, multi-client leases, claims, authenticated lifecycle IPC and atomic records.

Distinguish installation/account coordination identity, desktop instance ID, run generation, backend resource/generation, workspace/profile identity and native process identity. Use canonical resolved resource paths and explicit validated launch configuration for resource matching; a URL, saved PID or executable name alone never grants termination rights. Retain kernel process handles/identity where available and reject reused or unverifiable identities. Existing Python installation/account election and GUI-reload lease describe useful scoped contracts, not a global singleton or a distributed client count.

Allow concurrent independent SugarSubstitute desktops. A backend has one process-lifecycle authority; authenticated clients hold connection-bound leases at that owner. The supervisor serializes acquire/release/restart/stop and records each outcome. Closing or losing one client releases its lease without stopping a backend still leased elsewhere. An authorized frontend reload retains the same backend lease. After the last lease disappears, stop an owned backend through the bounded graceful/forced policy; attached servers remain external and are disconnected only. Restart of a shared backend requires an explicit user-authorized supervisor operation identifying affected clients. No automatic restart or inference replay follows a missed heartbeat or socket loss.

Profile exclusion applies only to the affected browser store, separately from application admission. L1 provides qualified engine-specific sharing/isolation. Give independent desktops usable distinct profile identities; a conflicting exclusive profile produces a typed contention result without starting another backend. Serialize shared authoritative settings/workspace writes through their owner and revision checks. Installation mutation exclusion remains a distinct future adapter boundary: this milestone adds no installer, updater, migration or mutation workflow.

Use private, access-controlled local IPC with kernel peer credentials where supported and per-run capabilities for privileged operations. Bound control messages to 64 KiB and outstanding requests to 64 per desktop; admission rejects overload observably. Version messages and include instance/run/resource generation, operation/request ID, deadline, typed outcome and duplicate handling. Keep capabilities out of logs, browser content and unrelated child environments. Authenticate registration, lease changes, completion and restart; reconnecting to a new generation cannot act on its predecessor. Only a controlled activation operation is publicly routable. Register implemented cross-version protocols and runtime/incident records with their authoritative persistence owner.

Publish runtime records atomically with schema and complete ownership generation. Treat records as hints for live-owner reconciliation, not authority or proof a process died. Clear only the matching generation; corrupt/stale/unknown records do not permit killing an unrelated listener or deleting authoritative data. Acquire native resource claims before publication. Locks protect exclusion; explicit transactions protect interrupted writes. Release through the owning lifetime, preserve stable lock-file identity while held and distinguish abandoned ownership from completed work. Standard Rust file locking is available on the pinned toolchain, with platform semantics qualified at the adapter.

### 3.3 M1.3 — Implement containment and explicit terminal outcomes

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement qualified native containment and typed terminal outcomes.

Windows assigns owned desktop/backend families to kill-on-close Job Objects at process creation, with an explicit inherited-handle allowlist and tested nested-job constraints. Owner handles are noninheritable; intentional independent handoffs require narrowly reviewed authorization. Avoid an unprotected create-then-assign interval. Crash capture must remain available through fault capture, with its registration/lifetime arrangement independently tested rather than inferred from breakaway flags.

Linux/macOS guardians own child launch/process groups and monitor an owner-held pipe. Only intended pipe ends cross creation; an accidentally inherited writer must not keep the owner alive. Explicit stop, supervisor EOF and root-child exit resolve remaining owned descendants and reap direct children. Guardian loss is an observed failure with bounded supervisor cleanup; do not claim process groups contain descendants that deliberately leave them.

Linux can use delegated cgroup v2/pidfd facilities when available; the baseline requires no privileged service. macOS native exit observation supplements the guardian and does not itself terminate descendants. Every platform records the actual containment guarantee and limitations. [Windows jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects), [Linux cgroup ownership](https://docs.kernel.org/admin-guide/cgroup-v2.html), [macOS process observation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kqueue.2.html).

Model normal completion, explicit cancellation, startup failure, unexpected termination, native fault evidence, suspected UI unresponsiveness, webview failure, backend exit and connection loss separately. Expected operation errors return typed failures; an unexpected panic escaping an authoritative owner triggers a fatal lifecycle outcome rather than silent continued use of potentially inconsistent state. Native faults are not recovered by catch-unwind. A successful kill request is not proof of termination, and dropping a future/handle is not proof of joined work or descendants gone.

### 3.4 M1.4 — Establish external crash observation, startup and shutdown transactions

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement external exit observation, UI watchdog and bounded startup/shutdown transactions.

Prepare a fresh authenticated run context and early bounded diagnostics before starting the desktop. Establish supervision immediately; L5 registers native capture before fallible production initialization. The supervisor observes the real process exit outside its UI/event loop. Accept normal completion only with matching current-run intent/completion evidence and the expected exit status. A zero exit without valid completion remains unexpected termination; explicit startup cancellation and known readiness failure retain their distinct attribution. Failed incident publication/reporting stays observable and retryable.

Startup obtains the first visible splash frame, then resolves launch/attach, acquires the backend lease/claim, establishes contained ownership, checks server readiness and creates/reveals the usable frontend. Retiring the splash is a presentation action. Normal shutdown stops admission, freezes state and invalidates generations, persists owned state, releases the client lease, conditionally stops the last-leased owned backend, tears down web/native resources and joins scoped work before publishing accurate completion.

Desktop-exit proof and final supervisor resource-settlement proof are distinct; the supervisor can record cleanup failure even after a desktop exits normally. Use one shutdown-transaction deadline with the existing 10-second graceful plus 5-second force/reap/join bounds; L3 consumes that budget instead of resetting it per component. Failed/uncertain outcomes retain verified ownership for retry and never fabricate clean completion.

Use an actual UI-event-loop challenge for suspected hangs: at most one pending challenge per visible active desktop, a 5-second cadence and an initial 30-second suspicion threshold. Suspend challenges during acknowledged OS sleep/native modal restrictions and rebase after resume; qualify scheduling delay under contention. Suspicion produces diagnostics and a user-controlled recovery option, not automatic termination. Inference silence, hidden-page JavaScript throttling and a worker heartbeat are not UI-crash evidence. Process death uses native exit events, without expensive periodic process-tree scans.

### 3.5 M1.5 — Parallel foundation work and acceptance

**Implementation:** Not started. **Verification:** Pending.

- [ ] Integrate the parallel supervision components and run real multi-instance/owner-loss/failure fixtures.

| Parallel M1 assignment | Exclusive ownership | Required output |
| --- | --- | --- |
| M1-A authority | Application lifecycle/resource state machine, identities, leases and tests | Multi-client acquire/release/reload/restart/stop invariants and typed outcomes |
| M1-N native lifecycle | Platform claims, process identity, containment/guardian adapters and native tests | Creation-time containment, owner/guardian loss and descendant/reap receipts |
| M1-P protocol | Private lifecycle IPC, bounded validation, authenticated completion and atomic records | Peer/capability, generation/replay/overload/corruption rejection proofs |
| M1-Q qualification | Controlled child/fault fixtures and public observations; no runtime-owner edits | Concurrent desktops/resources and synchronized death-at-launch/shutdown scenarios |
| Lead integration | Executable modes/composition, shared exports/manifests/catalogs/governance | One runnable fixture-supervised application and reviewed dispatch contracts |

Freeze shared lifecycle/identity/outcome contracts before parallel adapter implementation. M1 owns authority and generic containment; L3 later supplies Comfy-specific launch/readiness policy through those ports. L5 capture research/fixtures can proceed independently, but production integration and acceptance use accepted M1 ports. Every assignment has exact paths and a reviewer; freeze exports centrally rather than introducing forwarding shims or competing process managers.

Prove concurrent independent instances and distinct profiles; concurrent leases on one backend; release of one client while another remains; frontend reload retention; last-client cleanup; preservation of attached/foreign listeners; authenticated clean completion; hard exit including code zero; supervisor/guardian death; root exit with surviving descendants; cancellation at claim/spawn/publication/ready boundaries; generation/PID reuse rejection; noisy/inherited pipes; deadline escalation and retryable uncertain cleanup. Use real synchronized fixture processes for native guarantees and deterministic clocks for policy. Before each platform's dependent integration, record its native receipt; source inspection is not a containment receipt.

**Acceptance:** Governed supervisor modes, authenticated lifecycle protocol, scoped multi-instance/resource ownership and controlled native containment/failure proofs pass on the receiving platform. Shared ports and exact lane paths are frozen; other-platform receipts remain pending until run. L1-L5 can start dependent integration in parallel. Crash capture/symbolization and real Comfy/browser joint acceptance are L5/K1 obligations, not claimed by fixture supervision.

## 4. L1–L5 — Build startup, splash and unchanged frontend in parallel

**Entry:** M0 and M1 accepted on the receiving platform. **Owner:** Five exclusive lane owners plus the integration lead. **Deliverable:** Components for K1; independent development against the accepted ports.

### 4.1 L1: Native main window, webview and browser services

**Implementation:** Not started; reusable splash-window adapters are B7. **Verification:** Pending.

- [ ] Extend existing native adapters into a production main window with Wry, runtime/profile admission and browser services.

L1 owns native event loop, frame/hit-test/window commands, Wry lifecycle, profile, navigation admission, bootstrap readiness probe and browser failure hooks. Use Tao 0.37.0 and pin Wry 0.57.0 with reviewed native bindings/features and raw-window-handle 0.6 compatibility. Metadata declares Rust 1.85 minimums; complete transitive compatibility still needs compilation. [Tao metadata](https://crates.io/api/v1/crates/tao/0.37.0), [Wry metadata](https://crates.io/api/v1/crates/wry/0.57.0).

Host one primary webview at the configured ComfyUI root URL. Keep HTTP/assets/extension imports/WebSocket at that origin; no iframe shell, second Vue application, copied frontend or reverse proxy. Reserve a native caption strip above the webview for K1 so installed SugarCubes need not report drag regions or change styling.

Windows uses a normal desktop HWND, narrow reviewed subclass preserving Tao's procedure chain, documented DWM backdrop, theme/corner policy and themed solid fallback. Mica uses DWMWA_SYSTEMBACKDROP_TYPE/DWMSBT_MAINWINDOW on supported Windows 11 build 22621+; opaque Comfy content remains opaque. Preserve native resize edges, work-area maximize, native system menu, taskbar/Alt+Tab, Alt+F4, keyboard commands, double-click maximize and HTMAXBUTTON/Snap behavior. A message-loop hook alone does not cover synchronously sent window messages.

[Backdrop API](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type), [Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica), [custom frames](https://learn.microsoft.com/en-us/windows/win32/dwm/customframe), [Snap Layouts](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu).

macOS uses the native application/window/view lifecycle, standard close/minimize/fullscreen controls and keyboard/focus behavior. Linux uses Tao's GTK container and Wry GTK embedding for X11 and Wayland; generic raw-handle build_as_child only covers X11. Keep one UI event loop with platform initialization and toolkit affinity. Qualify the native titlebar/frame conventions, signed monitor positions, scaling and accessibility rather than copying Windows assumptions. Provide a macOS application/edit menu and clipboard accelerators; use native traffic-light/fullscreen conventions with the frameless content arrangement. [Wry embedding](https://docs.rs/wry/0.57.0/wry/).

Windows WebView2 is created/used on its STA UI thread with a message pump. Wry 0.57.0 synchronously pumps during environment/controller construction; invoke that constructor at an event-loop boundary, outside a WebView2 callback, and guard reentrancy. Test cancellation/close/retry during this pumping and discard stale constructed objects. A timer or cancelled future does not interrupt an uncancellable constructor. Native setup must not wait for a worker on the UI thread. [Thread affinity/reentrancy](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/threading-model), [published constructor source](https://crates.io/api/v1/crates/wry/0.57.0/download).

Preflight Evergreen WebView2 on Windows, system WKWebView on macOS, and GTK3/WebKitGTK 4.1 with WebKitGTK 2.40+ on Linux. Missing runtime produces native actionable recovery, without auto-installation. Keep standard privilege. Retain WebContext for the webview lifetime. Windows/Linux profile directories live in writable per-user application storage, separate from executable/checkout/everyday browser profiles. Coordinate each profile through M1's resource identity. Concurrent independent desktops with distinct stores are supported.

If a store is configured exclusive, a conflict produces scoped profile contention before acquiring a backend lease; it does not exclude independent application instances. Share a store only through qualified engine-supported behavior and compatible environment options; WebView2 supports sharing across application processes. An instance closing does not own the shared browser's exit or grant permission to delete its store. Qualification uses isolated owned profiles. Preserve browser-managed state; application metadata is catalogued, and the entire browser profile is not disposable cache.

[Runtime distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution), [profile lifetime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder), [WebContext](https://docs.rs/wry/0.57.0/wry/struct.WebContext.html).

The milestone's macOS persistent-profile path uses macOS 14+ and Wry's public with_data_store_identifier API, with a stable application-owned UUID stored in reviewed profile metadata. WKWebView does not honor the portable data_directory argument. Windows/Linux use scoped directories; macOS uses its identified website store in the application bundle identity. Record the minimum OS in build/runtime metadata; an older system may reject the bundle before application code runs, so provide the supported-OS information through bundle/distribution requirements rather than promising an in-app dialog there; do not silently use another store or claim older macOS qualification. [Identified Apple store](https://developer.apple.com/documentation/webkit/wkwebsitedatastore/init(foridentifier:)).

Admit exact canonical scheme/host/port, with no URL credentials, external privileged redirects, file/javascript URLs or arbitrary protocol execution. Inspect top-level and frame navigation. Reject unsolicited popups and unused device permissions. External HTTP(S) links open through a validated explicit user action, not redirect side effects. K1 exposes only its bounded ready/failure receipt. Native operations are never exposed through general filesystem/shell/process/evaluate-JavaScript proxies.

K1 uses host-owned native top-document script evaluation to obtain readiness, without installing a page-to-native command channel. Validate native current origin/navigation generation before scheduling and before accepting each callback, with one pending evaluation and the 4 KiB receipt bound. Windows ExecuteScript, WKWebView main-document evaluation and WebKitGTK evaluate_javascript provide the platform path. Exceptions, failed evaluation or stale results are failures, not ready. The read-only probe checks navigation success, mounted usable frontend and the installed post-setup app/graph plus SugarCubes public API.

Inspected frontend GraphView assigns window.app/window.graph after setup; installed SugarCubes exposes window.SugarCubes/openLibrary. Treat these as build-specific evidence and bound the probe; do not require a new SugarCubes handshake. Page-load events/script presence alone do not establish functionality. Verify ordinary frontend and library/editor/actions separately.

Wire native navigation failure, renderer/browser failure and event-token removal. Windows uses its public WebView2 interfaces for ProcessFailed and browser-process exit; macOS/WebKitGTK use their supported process-termination/load-error hooks. Freeze recovery before launching another generation, invalidate callbacks/jobs, release controller/context resources in order and preserve the profile.

Synchronize Windows browser-resource recreation with the relevant failure/exit events rather than an arbitrary delay; with a shared environment, one desktop closing must not wait for browser exit caused only by all clients closing, and profile deletion/runtime-wide changes require coordination of all affected clients; distinguish auto-recovered GPU/utility exits from main-renderer/browser failure. [Process recovery](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-related-events). Do not use automatic periodic reload or restart/replay inference as leak recovery.

Qualify clipboard, Unicode/IME, focused editing, Comfy shortcuts, file inputs/downloads and graph JSON/PNG external drop. Do not enable Wry's native drag/drop handler over the graph when it suppresses WebView2 default drops. Enumerated host accelerators must not steal editor commands. Downloads get a user-selected controlled destination, bounded lifecycle and failure feedback. macOS completion may not supply a saved path; use supported success/lifecycle evidence and the owned destination rather than assuming portable callback fields.

Development tools are explicit dev configuration; production has no remote inspection port or broad GPU/security flags. [Builder/drop/script APIs](https://docs.rs/wry/0.57.0/wry/struct.WebViewBuilder.html), [WebView2 security](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security), [native WebView2 interface](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2?view=webview2-1.0.3650.58).

### 4.2 L2: Production native splash and bounded transcript

**Implementation:** Not started; reusable splash is B4–B6. **Verification:** Pending.

- [ ] Extend the existing splash with bounded live transcript, native accessibility and production startup events.

L2 reuses the accepted S0 renderer and frame authority. Add the production styled read-only console as its own component: text shaping/font fallback, selection, scrolling, copy, keyboard navigation/focus and native accessibility semantics. Keep transcript retention/parsing separate from painting; it is not a terminal emulator. Preserve supported ANSI color/bold spans, split UTF-8/escape sequences, carriage-return updates and independently animated wait status.

Limits: 2,000 retained lines, 8 MiB retained UTF-8 text, 64 KiB per record and 1 MiB queued output. Continuously drain both owned stdout/stderr even when output is hidden. Trim oldest records and visibly report truncation/drop counts. Bound parser/shaping work per batch through execution; producer floods cannot delay close/cancel/control or animation. Qualify at 1 MiB/s combined fixture output and overflow beyond it, preserving bounded memory and feedback.

Match collapsed/expanded character placement, lower status/progress, console toggle beside close, progress at the top of expanded output, theme/accent/DPI and draggable passive regions. Disclosure never relaunches the server or loses retained diagnostics. Failure stops activity, exposes output and offers retry/cancel. Completion is monotonic producer-reported phase work, distinct from log activity; no fabricated percent from lines/time or omitted legacy provisioning. Reduced motion affects presentation, not lifecycle.

Application owns language/theme preference/OS fallback; all new native messages and control names have complete coverage for en, zh-Hans, ja, ko and es through one authoritative registry. Preserve foreign/user/cube output exactly. Native accessible controls and console selection must remain usable under high contrast, disabled transparency, inactive state and keyboard-only use.

L2 tests scripted events against the real view and startup events against an in-memory sink independently. Renderer/fonts/native types cannot enter startup/domain state. Normal splash retirement is a distinct presentation command; closing it requests cancellation. The executable is not a SplashApp that owns everything.

### 4.3 L3: Trusted launch/attach, readiness and cleanup

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement trusted Comfy launch/attach/readiness/retry through M1 leases and containment.

L3 owns Comfy-specific startup orchestration, explicit connection mode/configuration, launch arguments, diagnostics/readiness and cancellation/retry integration. It acquires backend leases and requests launch/stop through M1's authority and containment ports. It does not create a second process-lifecycle owner or implement competing shutdown bookkeeping. State sequence is SplashFirstFrame → PreparingConnection → ServerReady → WebviewLoading → FrontendReady → Running, with Failed/Cancelling/Stopped transitions. First native splash frame precedes Comfy launch/network work and webview construction.

Launch uses the explicitly trusted root, existing interpreter, working directory and argument array, loopback listen/port and --disable-auto-launch. Validate executable/root/main.py, reject conflicting managed listen/port arguments, inspect installed frontend and served extension identity, and never invoke a shell string. Resolve and validate the selected installation's existing interpreter/environment; preserve it. Paths remain ignored/local. No install/update/rebuild of ComfyUI or SugarCubes belongs to K1.

Attach connects only to a configured loopback HTTP root and never acquires process ownership. Do not silently attach after a launch port collision. A responding unrelated server is not proof that our child became ready; combine launch/process identity, expected port/origin and response validation. Bound HTTP readiness rather than waiting for a console phrase. Attach diagnostics cannot reconstruct another process's stdout/stderr.

Deadlines: 2-second probe, 30-second attach, 180-second owned launch, 30-second frontend readiness and 10-second graceful owned shutdown followed, if needed, by at most 5 seconds for controlled force/reap/join cleanup. Use monotonic scoped deadlines, bounded poll backoff and immediate child-exit detection. Retry creates a fresh generation after prior resources settle; never overlap owned launches accidentally.

Use M1's qualified Job Object/guardian containment and retained process identities. Acquire or join the matching managed-backend resource before launch; reuse requires live-owner validation. Unknown/foreign listener ownership remains a typed collision, not implicit adoption. Retry cannot overlap a previous generation's unresolved cleanup. Closing one desktop releases its lease; another live client's lease prevents backend termination. Cancellation of an attach operation releases only its connection. Real Comfy fixtures extend M1 proof for graceful/forced exit, early failure, inherited pipes, noisy descendants and cleanup. Attached servers remain alive on cancel/close/retry.

M1's application lifetime owner coordinates desktop/backend leases and terminal resource policy; the desktop coordinates its two native windows, webview and UI scopes through acknowledged lifecycle events. During normal handoff it retires the splash without cancelling the server/main host. After server readiness, construct the actual-size main window/webview and keep a lightweight native cover until usable frontend readiness. Do not rely on requestAnimationFrame from a permanently invisible browser. Test covered initialization and first usable frame/focus without flashes of blank/error content.

### 4.4 L4: Prepare independent qualification in parallel

**Implementation:** Not started; reusable tools are B8. **Verification:** Pending.

- [ ] Extend existing parity/native observation tools with independent startup, process and fault qualification.

L4 owns fixture process/server scenarios, platform observation/control and rendering parity receipts. Use public behavior, bounded observable waits, controlled clocks and fault injection. Fake only external boundaries; final hosting is actual Comfy frontend with installed SugarCubes.

Cover occupied port, invalid interpreter/root/runtime, slow/no HTTP, server early exit, high-volume/split output, extension absence, navigation error, pumped initialization cancellation, renderer failure, reload, profile contention, stale replies, every startup phase cancelled, normal handoff and graceful/forced close. Extend M1 fixtures for simultaneous desktops and shared/independent backends, loss of desktop/supervisor/guardian/capture/reporting roles, hard zero exit, startup/capture ordering, late completion, authentication/generation abuse, descendants after root exit, suspect UI stalls under contention and OS sleep/resume. Verify fixture cleanup without terminating unrelated processes. Confirm chosen tests actually ran and assertions fail on deliberately broken fixtures.

### 4.5 L5: Native crash capture and independent incident reporting

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement native crash capture, bounded incident persistence, symbolization and independent reporting.

Use Crashpad through a narrow reviewed Rust/native bridge as the first-checkpoint capture implementation, retaining the existing small C ABI design without Python or Qt runtime dependencies. Pin the packaged source/build provenance and architecture requirements; register runtime files/licenses/FFI owners before release. L5 owns capture registration, incident storage/reporting and release-symbol association; M1 alone owns external exit classification and restart authorization. [Existing bridge reference](../SugarSubstitute/native/crashpad/crashpad_client_bridge.cc), [Crashpad design](https://github.com/chromium/crashpad/blob/main/doc/overview_design.md).

Keep the handler outside cleanup that can kill it before capture completes. Qualify registration and capture together with M1's actual native process arrangement, nested jobs and POSIX lifetimes; allowing Windows breakaway does not prove the handler requests it. Capture-handler failure is explicit degraded diagnostics; it does not disable external termination observation. The reporting mode starts independently of ComfyUI/Wry availability, uses localized native presentation, retains pending evidence when presentation fails and prevents recursive reporting/restart loops.

Publish incidents atomically with instance/run/build identity, exit attribution, bounded startup/fault/log tails and native evidence when available. Initial retention policy permits 256 MiB of retained diagnostic payloads, 32 full incidents and 128 summaries of at most 64 KiB each; text attachments are capped at 256 KiB each. Acknowledged payloads expire at 90 days and retain at most 20 full incidents within those overall limits. Reclaim acknowledged payloads first, then the oldest pending payloads under pressure, retaining their summary/discard disposition within the bounded summary inventory.

Record summary eviction counts; never silently lose a pending incident. Quarantined records count toward retention, and temporary capture disk costs are measured separately. These are initial reviewed limits, not observed footprint claims. Do not classify missing dumps as proof of no crash, or unexplained termination as a confirmed native fault. Distinguish expected cancellation/readiness timeout from an application crash. Keep user settings, sessions and outputs outside diagnostic retention.

Collect locally with uploads disabled. Native dumps can contain sensitive memory; text redaction is not dump sanitization. Reporting exposes deliberate export/consent and retained-artifact disposition. Keep capture work small on the faulting thread; do not depend on allocating/logging/persisting through potentially broken runtime state. Register supervisor diagnostics as well as desktop capture before their fallible production initialization. Preserve exact PDB/dSYM/DWARF build symbols separately from developer binaries, and prove a real fault produces a symbolized report identifying the release and failing frame. The release line-table profile alone is not a qualified symbol pipeline. [Symbol pipeline reference](https://github.com/rust-minidump/minidump-pipeline).

Tests cover authenticated completion, startup cancellation/failure, panic/abort/native fault, hard zero/nonzero exit, handler unavailable/lost, capture completion before containment cleanup, corrupt/disk-full incident storage, retention saturation, reporter failure, and one actionable pending incident per failed run. Native qualification uses actual faulting helper processes and packaged runtime/symbol artifacts on each platform; synthetic dump files and source-reviewed Python tests cannot replace it. M1 fixtures and L4 fault orchestration may run in parallel once shared run/outcome contracts are fixed.

**Acceptance:** Each lane passes focused behavior/blast-area checks and reviewed native evidence for its platform. L1/L2/L3/L5 actual integration receipts depend on their implementations; L4 fixture preparation is parallel. Lead reviews cross-lane state/ownership before K1. No SugarCubes source/build changes or canvas work is required here.

## 5. K1 — Integrate and hand off the first working application

**Entry:** M0/M1 accepted and L1-L5 owner-tested components/individual native proofs ready for the receiving platform; this stage establishes their joint application acceptance. All required platform receipts are tracked separately. **Owner:** Integration lead and independent qualification reviewer.

**Implementation:** Not started. **Verification:** Pending.

- [ ] Integrate the real supervised Comfy/SugarCubes host and demonstrate the complete K1 acceptance matrix.

Compose the real application without replacing accepted owners. Start SugarSubstitute, show the native character splash promptly, launch the selected existing ComfyUI installation, display real progress/output, then reveal the native frameless window containing its actual frontend with installed SugarCubes. Windows Mica is visible in native chrome/splash with a solid fallback; existing content styling remains unchanged.

| K1 acceptance area | Required direct proof |
| --- | --- |
| Splash | All poses/animation, blank-to-live console, collapse/expand retention, loading/activity/failure, 1:1 reference, locale/theme/accent/DPI, selection/scroll/copy, keyboard/accessibility and reduced motion |
| Main window | Native move/resize/minimize/maximize/restore or fullscreen, mixed DPI/signed monitor coordinates, caption/input separation, focus/IME/keyboard, taskbar/window menu; Windows Mica/fallback/Snap |
| Existing frontend | Successful post-setup readiness, existing SugarCubes library/editor/actions, graph/workflow tabs, JSON/PNG drop, file input/download, clipboard and shortcuts, reload/process-failure recovery |
| Lifetime | Real owned launch and explicit attach; every-phase cancellation/retry, early exit/collision, both native windows closing during initialization, bounded noisy diagnostics, backend-lease handoff/reload, verified owned-tree shutdown and attached-process preservation |
| Supervision/instances | External crash observation, authenticated desktop completion plus supervisor resource settlement, hard zero exit, concurrent distinct desktops/profiles/backends, shared-backend lease release without stopping another client, owner/guardian loss and retryable uncertain cleanup |
| Native diagnostics | Capture registered before production initialization, actual panic/abort/native-fault evidence, correct release/frame symbolization, handler/containment ordering, retained incident when reporter fails and native reporting without hosted frontend |
| Security/profile | Standard privilege, correct runtime/profile identity, scoped profile contention with successful independent instances, authenticated local lifecycle admission, top-level/frame admission, malformed receipt, popup/protocol/redirect rejection, listener/token cleanup |
| Performance | External visible first-frame/interactive timing, UI/input percentiles, idle/static sleep, contention/drag/frame cadence, bounded queue/output memory, separate browser/host/Comfy costs |
| Reproducibility | Executable runs without legacy desktop or external asset paths; exact build/launch/config schema and served frontend/extension/runtime identities captured |

K1 is an independent developer handoff. Record executable identity, invocation and ignored configuration shape in its receipt now; do not delay it for generated TypeScript, host negotiation, canvas styling or mock images. Portable builds are insufficient to mark native acceptance. Keep macOS, Windows and Linux X11/Wayland receipts explicit; platform-independent implementation may proceed while missing platform qualification remains pending.

**Acceptance:** The receiving platform's first working launch is demonstrated and reproducible; full K1 acceptance requires every required platform/native matrix result. C0 design/portable fixtures may proceed from the working checkpoint; dependent platform mounting cannot be accepted on an unqualified host. Ordinary SugarCubes/Comfy repositories remain unchanged by K1.

## 6. C0 — Freeze desktop integration contracts after the working host

**Entry:** K1 working handoff and source/artifact baseline; dependent platform receipts remain explicit. **Owner:** Integration lead and one SugarCubes composition owner. **Deliverable:** Versioned Rust/TypeScript wire contract, shared validation corpus and immutable static-surface boundary.

### 6.1 C0.1 — Establish later integration ownership and public provenance

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement runtime-independent integration contracts and source-specific queue provenance at supported boundaries.

Retain accepted execution/supervision/containment/lease/crash-reporting/startup/frame/profile/splash owners. Add sugarsubstitute-mock-canvas with real static rendering/lifetime behavior; application owns document inventory/activation and adapters own transport/presentation. Contract declarations are canonical Rust, emitted into TypeScript through a Rust-driven ts-rs task and checked by artifact fingerprint/parity command. Runtime validation on both sides uses the same positive/negative corpus; TypeScript declarations alone are insufficient. Register protocol version 1 and its compatibility/source fingerprint in governance/persistence/catalog.toml.

In SugarCubes one composition owner owns frontend/comfyui/ui.ts, extension setup and existing execution composition callsites. P3/P4 supply focused modules rather than independently editing root registration or installing competing queue wrappers. Its own AGENTS, package scripts and declared test policy govern those changes. No frontend application/toolchain is introduced in SugarSubstitute.

Use the deployed 1.47.11 frontend's supported public app/extensionManager interfaces and compatibility shims. Its available source maps reveal app.extensionManager.workflow and queue behavior that the older separate checkout/types cannot establish. Add a focused typed deployed-API adapter with contract tests and explicit compatibility failure. Do not import private Pinia stores, scrape executionStore internals, patch compiled bundles or use a private source-map path at runtime.

Ordinary queue provenance must be captured when the actual workflow snapshot is taken, before asynchronous graphToPrompt. The deployed app captures queuedWorkflow at that point, before its existing api.queuePrompt call; current workflow at the SugarCubes API wrapper is too late. The existing public graphToPrompt boundary can associate its input graph and returned prompt object with a scoped immutable workflow/run context, while the existing queue owner associates the eventual receipt.

Integrate observation once at the supported projection/queue owner, retain per-item object/request context rather than a global last-run variable, and propagate it through relation preparation/transform pipelines that return replacement payload objects. Freeze the actual projected/submitted snapshot identity while retaining its pre-projection workflow/selection context; indexing only the original object is insufficient. Capture queued-but-not-yet-projected items at their actual snapshot/projection boundary, consistent with Comfy's queuedWorkflow semantics, rather than assigning the workflow from an earlier promptQueueing event.

Preserve signature, this-binding, ordering, transform and error semantics. Contract-test batched/concurrent invocation and tab changes during projection. A generic promptQueueing/promptQueued event lacks prompt/workflow identity and is not a complete receipt. Ambiguous/unregistered provenance receives an actionable unsupported-result diagnostic rather than being assigned to the active tab. Direct Cube execution captures equivalent context before serialization and serializes that same captured graph/snapshot without re-reading mutable active graph/workflow after asynchronous work.

### 6.2 C0.2 — Freeze the envelope and operations

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement the canonical envelope/operations, generated TypeScript and bounded shared validation corpus.

| Version-one contract | Required fields and semantics |
| --- | --- |
| Envelope | Protocol version, host session, navigation/document epoch, request ID, operation discriminator and typed payload; reply carries matching identity/result/error |
| Handshake | host.hello/client.ready with product/protocol range/platform, effective theme/backdrop and nativeWindow/staticCanvas/canvasTabs capabilities |
| UI readiness | interface.ready after SugarCubes setup, usable Comfy APIs and desktop root mount; served identities and current applied layout revision |
| Window state | Host-owned maximize/minimize/theme/DPI/backdrop, reserved native caption bounds; narrowly enumerated acknowledged window commands and validated drag/no-drag regions |
| Surface layout | Mount ID, increasing layout sequence, host geometry epoch, CSS viewport rectangle, viewport/device scale, visibility; acknowledgement reports applied sequence and clipped physical bounds |
| Workspace | Revisioned full snapshot of IDs, workflow/run associations, load/error state and active ID; select/close against expected revision; stale command receives a fresh snapshot |
| Queue receipt | Run ID, opaque source workflow key, foreground-selection epoch, snapshot/request identity and accepted prompt_id; failed/rejected attempt creates no result |
| Output offer | Registered prompt/run, full execution node ID, ordinal and exact filename/subfolder/type; no URL/local path/native handle/base64 pixels |
| Image/prepared frame | Scope/run/document/import/geometry generation, cancellation and admitted immutable validated pixel lease; typed error or stale suppression |
| Detach/recovery | Explicit navigation invalidation/detach, scope cancellation and callback/lease release; late messages cannot revive an old epoch |

IDs are bounded opaque strings; counters are nonnegative JavaScript-safe integers; geometry is finite with explicit units. Reject unknown versions/operations/fields, malformed types and borrowed native addresses. Use structured JSON serialization and platform message delivery; Windows uses PostWebMessageAsJson rather than concatenating data into executable script. Other platforms use the minimal reviewed JSON-serialization transport available in their native adapter.

Limit controls to 64 KiB and 64 pending control/output metadata items, 256 registered runs and 32 open documents. Reserve capacity before accepting work. Image admission permits one decode at a time, at most eight waiting image jobs within that metadata capacity, 64 MiB compressed bytes, 32 megapixels, 128 MiB decoded pixels per image and 256 MiB aggregate managed image storage. Account compressed/staging/decoder scratch/decoded/resampled/presenter copies and queued/in-flight frames; shared leases count once, independent copies separately. Peak reservation precedes allocation. Limits are initial development policy, not measurements; reject overload visibly rather than silently deleting another document or losing outputs.

Keep accepted startup deadlines. Add 30-second desktop negotiation/setup, 30-second image transfer and 5-second ordinary IPC request deadlines. Cancellation/retry starts a new generation. Do not replay prompts/restart inference or retry indefinitely. Wry construction has its separately proved native cancellation semantics; IPC timeouts do not cover it.

### 6.3 C0.3 — Implement actual platform frame admission

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement platform-native frame/origin admission and bounded asynchronous transport.

Wry's IPC request URI is not portable sender-frame identity. Windows uses the scoped top-level CoreWebView2 WebMessageReceived event and its native Source; no child-frame command handlers are registered. macOS installs an application-owned WKScriptMessageHandler checking frameInfo.isMainFrame, native security origin/current document and generation before dispatch; the generic Wry adapter does not forward all that metadata.

Linux does not authorize generic GTK script-message payloads: Wry constructs their URI from the top-level webview, even if a subframe calls the registered endpoint. Use a host-owned native main-frame pull bridge for this static-canvas milestone. SugarCubes enqueues bounded commands in the top document; a scoped native GTK UserContentManager callback supplies only an untrusted wake hint, never an operation or claimed origin. Do not enable Wry's generic IPC handler for this path: it converts arbitrary incoming values to allocated strings before our admission.

Treat native hint payload as opaque, do not stringify/parse it, and only set one coalesced dirty flag. Rate-limit scheduling before reading the top-frame outbox; browser-engine message receipt costs are measured separately. The desktop coalesces/rate-limits hints, schedules at most one native evaluate_javascript operation and reads a bounded top-frame outbox. Validate native origin/generation before and after evaluation, then validate the envelope. A foreign-frame hint cannot put a command into the top document; same-origin extension code remains within the trusted frontend context.

Bound Linux batches to the shared 64 KiB/64-item admission and at most one evaluation per 16 ms while hints/work remain; no idle poll loop. Retain bounded entries until acknowledgement, restore/retry only their current epoch, and bootstrap one drain after navigation/negotiation and schedule current-generation drains on restore/resume, re-negotiation and recoverable wake/evaluation disruption. A lost hint cannot permanently strand retained work. Invalidate a timed-out evaluation's operation ID; its late callback cannot complete a newer drain.

Reserve admission before outbox insertion; producers receive explicit busy/cancelled responses rather than silent drops. Maintain bounded per-session/epoch/request-ID dispatch deduplication and retained terminal replies until acknowledgement. Bind each request key to its validated payload identity and reject conflicting reuse. A repeated pull or lost ACK returns the same reply and never executes select/close/window operations twice; test duplicate batches, conflicting IDs and lost acknowledgements. Reject retired request identities and apply explicit admission when replay-retention capacity is full. Lifecycle receipts cannot disappear.

Prove forged/flooded hints, navigation during evaluation, about:blank/srcdoc/foreign/same-origin frames, process termination and current-document outbox replacement cannot enter stale/foreign privileged commands. The native source context establishes provenance, not a JavaScript guard/nonce.

Use WebKitGTK 2.40+ from the 4.1 API family for its public main-frame evaluation and cancellable result. Keep native transport lifetime/affinity in desktop and expose the same asynchronous wire semantics to SugarCubes on all platforms. [GTK main-frame evaluation](https://webkitgtk.org/reference/webkit2gtk/stable/method.WebView.evaluate_javascript.html), [GTK message metadata](https://webkitgtk.org/reference/webkit2gtk/stable/signal.UserContentManager.script-message-received.html), [WK native frame identity](https://developer.apple.com/documentation/webkit/wkframeinfo/ismainframe).

### 6.4 C0.4 — Freeze layout and rendering authority

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement the retained-frame/layout contract and selected native backing-plane arrangement.

Keep one primary frontend webview for all canvas tabs. The mock is a CPU-painted native backing plane beneath the transparent webview; SugarCubes makes only its reserved canvas slot transparent while tabs/controls/content retain readable backgrounds. Webview remains the mock's full input/overlay plane. No editing input is needed for a static canvas, and no native image window sits above Comfy dialogs/menus.

Comfy dialog services, graph menus, PrimeVue portals and SugarCubes dialogs have separate owners. Their visibility and input must remain correct through native composition. MutationObserver is too late to prove no obstructed first frame. Wry/WebView2 transparency APIs support the intended host-background model; P1 must prove the complete platform arrangement before native mounting acceptance. [Transparent host background](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controller2?view=webview2-1.0.2849.39), [airspace limitations](https://learn.microsoft.com/en-us/microsoft-edge/webview2/platforms/wpf).

Surface conversion has one desktop owner. Milestone webview page zoom stays at 1, with OS DPI and accessible text/controls preserved. Validate viewport scale/devicePixelRatio against actual webview rasterization, apply caption offset, round edges once and clip to client bounds; do not multiply both DPI and devicePixelRatio blindly. Reject stale DPI/layout epochs and hide zero/minimized/unmounted surfaces. Cover signed monitors left/above primary, mixed DPI, resize/maximize and scale changes. Coalesce layout to newest revision while preserving its acknowledgement; prioritise detach/close/failure above replaceable geometry.

Retain admitted pixels and repaint only on content/bounds/expose changes. CPU workers prepare fit/alpha frames; UI native paint submits immutable retained results. Windows uses the narrow DIB/GDI owner; macOS/AppKit and Linux/GTK presenters provide the same frame/lifetime contract. [Bitmap API](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-stretchdibits).

Use CuteCanvas's existing headless Editor/RasterProduct/DocumentRenderer/CpuRenderBudget and QPane immutable retained-frame public examples as contract references. Do not link qpane-qt, duplicate engine policy or invent speculative engine-editing types. The mock adapter remains replaceable through geometry, immutable-frame and lifetime ports.

### 6.5 C0.5 — Prepare parallel dispatch and validate compatibility

**Implementation:** Not started. **Verification:** Pending.

- [ ] Freeze lane paths/contracts and prove generated/runtime acceptance parity before dependent mounting.

The lead owns shared Rust/wire fixtures and both-repository artifact emission. Freeze exact native ownership paths, P3/P4 SugarCubes modules, P2 startup changes, P5 workspace/mock modules and P6 harness locations. Register new localization/persistence/cache/vendoring owners atomically when implemented; no duplicate text/locale inventories or incidental cache paths.

Prove Rust/TypeScript acceptance and rejection parity, generated-artifact drift detection, frame/workspace/request revision semantics, overload and deadline behavior. Freeze origin/frame admission on actual platform IPC metadata; namespaces, nonces and host flags do not authenticate SugarCubes separately from same-origin extensions. The entire selected extension environment is trusted; privilege remains narrow. Host awareness cannot be enabled by query string/user-agent alone.

**Acceptance:** Canonical types, shared corpus, generated parity and deployed-API contracts pass; all P1-P6 scopes have one writer. No platform composition is claimed proven by these contracts. Dispatch pure P2-P6 work and P1's native feasibility proof in parallel against this baseline; native mounting waits for P1 acceptance.

## 7. P1–P6 — Build native mounting, SugarCubes UI and static results in parallel

**Entry:** C0 accepted. **Owner:** Assigned lane owners. **Deliverable:** Checkpoint-two components, with native feasibility accepted before mounting integration.

| Parallel lane | Exclusive implementation/test ownership | Dependencies for final acceptance |
| --- | --- | --- |
| P1 native composition | Desktop backing-plane/frame/geometry/transparent-webview adapter and native scenarios | C0 and K1; controlled native feasibility before I1; real P3 composition is accepted in I1 |
| P2 startup/recovery | Application startup/negotiation and existing Comfy connection scopes | C0; owner-level state tests before I1, joint reveal/recovery accepted in I1 |
| P3 SugarCubes desktop | New host transport/lifecycle/UI/tabs/styling/layout subtree | C0; browser/fake-host contract readiness before I1, native joint proof in I1; lead edits root registration |
| P4 output integration | SugarCubes scoped projection/queue/output observers and Comfy image transport modules | C0; P3 host client/P2 transport ports; lead edits existing composition |
| P5 workspace/static mock | Application workspace modules and mock-canvas preparation/presentation | C0; P1 native attachment; lead owns exports |
| P6 proof harness | Deterministic process/server/image/workflow fixtures and public harness | C0; real I1/I2 application for final receipts |

Workers can develop against shared boundary fakes while collaborators are pending. They do not mutate each other's contracts or create temporary compatibility shims. Narrow interface changes go through the lead and update both consumers, corpus and ledger.

### 7.1 P1: Prove native composition before dependent mounting

**Implementation:** Not started. **Verification:** Pending.

- [ ] Prove native transparency, stacking/input and layout with a controlled page/bitmap before I1.

Establish P1 feasibility with a tiny controlled transparency page and prepared asymmetric bitmap. This receipt unblocks joint I1 integration; the actual hosted frontend/SugarCubes composition is accepted there, not required before its components can be integrated. Prove native backing-plane pixels appear through only the canvas slot, opaque web controls remain above them and the web input plane receives pointer/focus/keyboard correctly. Cover ordinary Comfy and SugarCubes dialogs, context menus, detached dropdowns, tooltips and menus crossing the canvas, including their first visible frame. DOM z-index assertions or browser screenshots alone do not establish native composition.

Exercise rapid resize/maximize, caption exclusion, DPI/display changes, minimize/restore, zero bounds, high contrast, disabled transparency, inactive theme and retained expose paint. Recheck splash/main cover/reveal with the new surface. On Windows verify DWM/native backing without a child overlaying WebView2; on macOS verify native view hierarchy/alpha; on Linux use GTK container composition and qualify X11/Wayland. Do not substitute a web img element for native proof.

If the chosen backing-plane implementation fails, correct its platform compositor adapter and repeat proof before accepting mounting. Record concrete failure and affected dependencies. Pure UI/output/workspace/harness work continues; no claims that CSS solves a failed native stack. Keep mock input in the webview; future interactive CuteCanvas input requires a later explicit contract.

### 7.2 P2: Add host negotiation and lifetime recovery

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement negotiated readiness, epoch invalidation, reload/resume and retained workspace recovery.

Extend accepted WebviewLoading with HostNegotiated → InterfaceReady before Running. Server readiness, desktop setup, usable frontend and applied empty native layout are independent gates; inference is not required. Keep native failure/retry presentation for absent/incompatible protocol, extension error, stale epoch and layout timeout. Preserve every K1 cancellation and normal handoff behavior.

Invalidate old client callbacks/import work on reload, retain the native workspace and supply a new full snapshot after negotiation. A new server connection gets a new connection epoch; never merge old pending work into it or replay inference. Distinguish frontend navigation/reload, renderer failure, server exit/socket loss and application shutdown. Pending tasks settle through the execution owner; process ownership remains with M1. Preserve the backend lease during authorized frontend replacement. Feed webview/connection/UI-progress events into the existing supervision protocol; do not create another watchdog, runtime or backend owner. Losing one frontend does not stop another client's backend or replay its prompts.

Handle minimize/background/OS sleep and resume explicitly. Native process/pipe drains do not depend on JavaScript timers. On resume, re-evaluate monotonic deadlines/connection status, restore only current frames and trigger registered-run catch-up. Browser throttling/suspension can lose event notifications; leaving the browser always busy is not recovery policy.

### 7.3 P3: Let SugarCubes own desktop presentation

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement negotiated SugarCubes desktop styling, canvas tabs and acknowledged layout/workspace projection.

Mount desktop UI through its existing extension setup lifecycle. Negotiate the injected host client before enabling capabilities; tear down listeners/observers/subscriptions on unload/navigation/dispose. Preserve ordinary-browser presentation when no host negotiates. Contract-test missing/incompatible/delayed/reloaded host and fake-host acknowledgement ordering.

SugarCubes owns WinUI/Fluent-style workflow tab presentation through the existing Comfy workflow owner, canvas tab controls, menus/panels and layout intent. Native chrome/caption state remains authoritative in Rust. Tabs and interactive controls are no-drag areas. Host styling is scoped to negotiated desktop mode; transparent canvas/chrome regions do not make all application content unreadable or change ordinary-browser CSS.

Project revisioned native workspace state. Tab clicks/close requests wait for acknowledged native state; stale revisions refresh the snapshot. Preserve keyboard selection, accessible names/focus order, long/localized titles, inactive/error/loading states and narrow/overflow layouts. The native host does not recreate this tab UI.

Observe layout through its single mounted owner; emit bounded coalesced latest revisions and actual visibility. Native canvas cannot cover tabs/caption because it is behind the web input/content plane. Menu/modal focus restoration and actual frontend portals remain P1/I1 tests, not assumptions about DOM layering.

### 7.4 P4: Preserve queue provenance and deliver exact registered outputs

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement both queue-path provenance and exact registered output delivery/catch-up/deduplication.

Comfy retains its existing WebSocket/clientId and queue. Rust neither queues inference nor opens a competing inference socket. Extend the single existing SugarCubes execution owner; never determine result source from whichever tab is active on arrival.

1. Capture immutable workflow/foreground context at each actual queued item's snapshot/projection boundary before asynchronous projection/serialization. Freeze the resulting submitted snapshot identity, propagate context through replacement transforms and direct serialization, and never re-read switched mutable graph state. Carry context through batch items and accepted prompt receipt without altering seed order, transforms, return values or errors. Rejected/failed queue attempts create no result documents.
2. Register accepted prompt/run with Rust. Buffer bounded early output metadata while receipt is pending; drain once registered, or expire/discard with diagnostics on failed registration. Cover cached results before HTTP queue reply and concurrent calls. Unknown/ambiguous prompt provenance is not imported.
3. Observe executed output.images for ordinary SaveImage/PreviewImage and full prompt-only Cube execution IDs. An executed message represents UI output, not every node completion. Use complete subgraph execution paths.
4. On execution_success, reconnect, resumed/rehydrated desktop or known missed notification, reconcile /history/{prompt_id} for exactly registered prompts. Poll only while an accepted run is unresolved with bounded backoff/deadline; do not import broad recent history into desktop tabs. [Comfy messages](https://docs.comfy.org/development/comfyui-server/comms_messages).
5. Normalize once at the execution adapter and dedupe by connection/session/run, prompt, full node ID, ordinal and exact descriptor. Event/history/reconnect copies create one document; identical filenames from different prompts are distinct results.
6. Rust constructs the encoded /view request from its configured origin and filename/subfolder/type. Accept temp/output only; reject absolute/drive/UNC paths, traversal, filename separators, invalid subfolder components, embedded Comfy directory annotations and unsupported types. Validate both separator styles without native filesystem resolution.
7. Fetch with bounded streaming, status/content checks, no redirects, cancellation and transfer deadline. Do not accept browser-provided URL/path/headers/credentials or ask Comfy preview recompression to conceal large images.
8. Pass admitted bytes/identity to the existing execution/frame preparation boundary. Normalize actionable failure feedback to SugarCubes; original output files are untouched.

The first connection mode is unauthenticated single-user local Comfy. Frontend fetchApi carries Comfy-User in multi-user mode; native transfer must not silently lose that identity. Preflight/reject unsupported authenticated/cloud/proxied/multi-user use before claiming output integration. Any later credential/user-context support requires a reviewed connection contract, not arbitrary IPC headers.

### 7.5 P5: Implement bounded immutable documents and CPU presentation

**Implementation:** Not started. **Verification:** Pending.

- [ ] Implement admitted static native documents, CPU decode/fit/alpha and authoritative workspace revisions.

Decode PNG/JPEG/WebP with only required decoder features after reviewed version/license/MSRV/lockfile selection. Inspect format/dimensions/allocation arithmetic and reserve aggregate peak memory before decode; enforce C0 compressed/dimension/pixel/scratch/copy limits. Verify content rather than trusting extension/MIME. Corrupt, unsupported, missing and oversized media produce visible per-result errors without crashing or deleting another document.

Create one immutable static document per admitted image, grouped with source workflow/run. Fit to viewport with aspect ratio, center/letterbox and alpha-composite against a neutral background. Preserve source pixels for retained workspace lifetime and derive bounded resampled frames; do not continuously resample/repaint static content. This mock claims ordinary decoded display pixels, not color-managed fidelity. No editing/zoom/pan controls or canvas history are advertised.

Application owns active document and inventory revisions. Activate a run's first result only when the user has not changed foreground selection since queueing. Other images/batches/background workflows add tabs without stealing focus. Close cancels/releases that document; event/history duplicates and late jobs cannot recreate it. Retire completed runs once documents close, preserve closed-result tombstones for the full lifetime of their registered run and reject retired/unknown runs. If bounded tombstone retention is exhausted, retire the affected run or reject further imports with visible feedback; do not evict a still-relevant closed identity and allow history replay to resurrect it. Revision/import/connection/geometry checks prevent stale worker publication.

Treat mock documents as in-memory state. Frontend reload retains them; application restart does not restore or translate sessions. All retained native/workspace/image leases must release on document/application close and failed admission, including platform copies. A document/buffer lease belongs to its desktop workspace; it is distinct from M1's managed-backend client lease and cannot authorize backend termination.

### 7.6 P6: Build deterministic integration proof

**Implementation:** Not started. **Verification:** Pending.

- [ ] Build isolated deterministic image/process/workflow fixtures and joint native observation scenarios.

Use first-party asymmetric color/alpha PNG/JPEG/WebP fixtures, corrupt/oversized/slow/chunked/redirected responses, controlled process trees and a public native observation/control harness. Build CPU-capable Comfy image workflows using existing nodes, including separate SaveImage, PreviewImage and Cube paths, with no model downloads or AI inference required. Real model runs supplement testing; they are not its reproducibility prerequisite.

Qualification instances have explicit isolated ports/profiles/user/input/output/temp roots and the correct built extension. Never share a user's live queue/storage between agents. Remove only verified fixture-owned files/processes. Fault-server tests cover transport failures, but final proof uses production Comfy routes.

Tests verify semantic pixels from known fixtures and actual native mounting/overlay/focus. Add regression-detecting assertions for tab/provenance races, cached early events, reconnect after missed success, duplicate history, late closed outputs and resource saturation. Portable tests remain parallel; native focus/timing/display scenarios serialize only their real shared resources.

**Acceptance:** Owner-level implementations and tests are ready for integration, with independent reviews. P1 has controlled native feasibility receipts before I1 on each platform. I1 establishes real joint P1/P2/P3/P5 acceptance; I2 establishes joint output acceptance. Other lanes retain explicit dependency/evidence status; code compiling against a fake host is not the integrated result.

## 8. I1 — Integrate desktop negotiation and the empty native canvas

**Entry:** P1 controlled native feasibility accepted; P2/P3/P5 empty-surface components are owner-tested and ready for integration; P6 harness available. Integrated acceptance is produced by this stage, avoiding circular prerequisites. **Owner:** Integration lead with lane owners.

**Implementation:** Not started. **Verification:** Pending.

- [ ] Integrate negotiated empty native canvas and prove real frontend layout, overlays, input and recovery.

First preserve K1 launch and splash, then add version negotiation, SugarCubes desktop root/tabs/layout and the acknowledged empty native backing surface. Inference and output import are not entry conditions. Verify local/attached launch, absent/incompatible extension/protocol, late readiness, retry, reload, native runtime failure and cancellation at every new phase. Extend K1 supervision receipts for negotiated frontend replacement: preserve backend leases, invalidate old callbacks, restore the same workspace snapshot, retain other instances and verify native failure reporting when negotiation cannot finish.

Prove empty/static surface bounds follow resize/maximize/DPI/signed monitor changes, canvas never covers caption/tab controls, actual web overlays stay visible from their first frame and focus/IME return correctly. Test null/zero/minimized/detached layout and stale acknowledgements. Verify browser mode remains unchanged when no host is negotiated. Keep K1 handoff usable throughout.

**Acceptance:** Real frontend with SugarCubes negotiates and shows an empty native canvas through the accepted composition; overlay/input/layout/recovery receipts pass. P4/P5 output work and portable P6 proofs can continue in parallel while I1 integrates. Record no image-delivery claim yet.

## 9. I2 — Integrate real Comfy outputs, native pixels and canvas tabs

**Entry:** I1 accepted; P4/P5 owner-tested components and P6 fixtures/proofs are ready for joint integration. This stage establishes actual output-slice acceptance. **Owner:** Integration lead, output/workspace owners and independent reviewer.

**Implementation:** Not started. **Verification:** Pending.

- [ ] Integrate real ordinary/direct Comfy outputs into native pixels and acknowledged tabs; preserve lifecycle and ownership.

Connect accepted queue context/receipts, exact descriptors, Rust streaming/admission/decode, immutable native documents, SugarCubes tabs and native retained paint. Run deterministic real Comfy workflows through the normal queue and direct Cube queue. SaveImage, PreviewImage and Cube image outputs each have their own receipt.

| I2 integration scenario | Required observation |
| --- | --- |
| Successful image/batch | Correct descriptor/prompt/workflow/run, semantic native pixels, aspect fit/alpha, correct number of documents and acknowledged tab activation |
| Provenance races | Switch workflow during graph projection, submission and output; queue concurrent/batched/direct runs; preserve captured snapshot identity or explicitly reject ambiguity |
| Early/cached/reconnected output | Register/drain bounded early offers; reconcile exact prompt after missed success/reconnect/resume; no duplicate document |
| Foreground policy | Background results remain associated; later results do not steal selection; closed/retired runs and stale completions never resurrect tabs |
| Transport/media | Unicode references, both separators, traversal/URL injection, redirect/status/MIME mismatch, slow/chunked/missing temp files, corruption/unsupported/oversized pixels |
| Resource policy | Queue/decode/document/byte saturation produces feedback; peak reservations cover all copies; rejected/closed jobs release resources and keep UI responsive |
| Reload/server changes | Native workspace survives frontend reload; new epoch receives snapshot; old callbacks/imports stop; new connection remains separate; no prompt replay |
| Native presentation | Retained expose/restore, rapid layout/DPI, web menus/modals/dropdowns over pixels, clipboard/IME/focus and no static animation loop |

Extend K1/I1 lifecycle proof under active imports: desktop/backend/renderer loss, shared-backend clients, document close and shutdown cancel only owned scopes, retain correct incident attribution, prevent stale document resurrection and release all frame/decode/buffer reservations. Closing or failing one instance must not interrupt another instance's inference or clear its shared queue; use registered prompt-specific cancellation where explicitly authorized. The final receipt uses production /view/history/WebSocket delivery and actual native pixels. A fake server, successful IPC, browser image or screenshot alone cannot prove that slice. Keep source output files unchanged. Record exact backend/frontend/SugarCubes build/protocol/runtime identities and native platform.

**Acceptance:** Output, tab, native presentation and failure/recovery scenarios pass on the integrated application; preserve K1/I1 regression receipts. Continue independent platform qualification and owner-level fixes; do not weaken assertions or resource limits to conceal failures.

## 10. Q1 — Qualify the complete application, including performance and cleanup

**Entry:** I2 integrated and all owner-level suites passing. **Owner:** Qualification lead, independent reviewer and individual failure owners. **Parallel work:** Isolated startup/process, protocol/security, SugarCubes browser, image/resource and portable suites run concurrently; shared native display/focus/timing scenarios run sequentially.

### 10.1 Q1.1 — Prove functional and platform behavior

**Implementation:** Not started. **Verification:** Pending.

- [ ] Run full functional/security/native lifetime matrices on each supported platform and fix failures.

Run the complete K1 and I1/I2 scenario sets on Windows, macOS and Linux X11/Wayland. Cover native caption/menu/focus/input/accessibility, transparency fallback, OS theme/high contrast/reduced motion, five locales, DPI/layout, split/flooded output, server/webview failure, profile contention and every cancellation/retry/reload/shutdown phase. Re-run the M1/L5/K1 fault matrix under integrated load: concurrent instances, shared/independent backend leases, UI stalls/OS sleep/resume, desktop/supervisor/guardian/handler/reporter loss, root exit with descendants, PID/generation reuse, disk pressure/corrupt records, authenticated versus hard-zero exit, capture cleanup ordering and exact-release symbolization. Fault qualification begins with each owner before K1; Q1 broadens it rather than introducing external supervision for the first time.

For security, exercise foreign origin/frame, navigation changes, popups/protocol launches, unknown/malformed/oversized/versioned messages, stale generation/request replay, invalid descriptors and unregistered run output. Same-origin extensions are within the configured trusted frontend; the client name/nonce is not independent authentication. Review every FFI lifetime/unsafe exception and native event-token/callback removal.

Run repeated launch/retry/reload/close cycles under active transfer/decode/raster/output. Observe no owned child, worker, native controller/view, GDI object, profile lock, callback subscription or buffer lease surviving its owning completion. A shared backend/browser or independent capture/reporting process remains only while its explicit other-client/capture/reporting lifetime requires it; verify its final exit after the last relevant owner releases it. Include supervisor/guardian/capture/reporting roles and lease counts in the observation. Keep attached servers alive.

Exhausting the 10-second grace interval enters forced cleanup; successful cleanup within the additional 5-second force/reap/join bound is an explicit forced terminal outcome. Remaining owned resources after the final deadline are a failure receipt, not successful close. Independent browser-constructor/API cancellation limitations remain explicit and cannot be hidden by this process deadline.

### 10.2 Q1.2 — Measure costs where they occur

**Implementation:** Not started. **Verification:** Pending.

- [ ] Measure startup, responsiveness, resource bounds, idle behavior and inference interference; resolve missed targets.

Measure cold/warm external launch through visible splash, interactive splash, server-ready, webview creation, usable frontend and reveal. Keep Comfy startup time separate from host overhead. Benchmark the same installed frontend in its normal browser and hosted webview; distinguish browser/runtime costs from our own rendering/execution. Native initial UI and a single primary webview follow the platform's performance guidance. [WebView2 performance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance).

Measure animated visible splash, idle running frontend, static active canvas, hidden/minimized window, console flood, drag/resize/mixed DPI, image batch and controlled CPU contention. Supplement with a real inference-load comparison. Include all product-owned supervisor/guardian/capture/reporting processes and concurrent desktop instances in aggregate CPU/memory/process measurements; distinguish them from Comfy and engine-owned browser processes. Record p50/p95/p99 UI/event/frame latency, queue wait, cancellation lag, memory peaks/reservations, CPU in units of one logical core, browser subprocess count and GPU/VRAM usage. Hardware inventory and full paths remain in ignored receipts, not tracked engineering docs.

Meet the S0/M0 latency targets on the documented uncontended setup and explain contention behavior separately. Initial host idle target is below 1% of one logical CPU core over a stable 30-second static interval, with no periodic native canvas repaint or expensive poll loop. Native image storage remains within 256 MiB including peak copies; other host/browser allocations are measured separately. Detect growth after repeated cycles rather than claiming process RSS must return byte-for-byte to baseline.

Browser acceleration remains normal. Do not introduce blanket disable-gpu flags, a separate application GPU renderer or “CPU-only webview” claims. Measure inference interference. An optional later OS graphics preference may help an available iGPU, but engine GPU-process device use must be observed; selecting the launcher preference is not proof. This is not a milestone blocker or a guaranteed cross-platform adapter binding.

When a target fails, identify the responsible owner/stage, fix the cause and repeat affected proof. Budgets may change only through explicit reviewed evidence and recorded rationale, never merely to make a check pass. Do not serialize portable tests, add retries, skip applicable platforms or mask out UI regions to hide nondeterminism.

### 10.3 Q1.3 — Run the established repository gates

**Implementation:** Not started; repository gate implementations are B2/B3. **Verification:** Pending.

- [ ] Run existing Rust gates and affected SugarCubes gates; complete runtime/CI declarations and record actual results.

SugarSubstitute checks are cargo xtask governance, cargo xtask check and cargo xtask verify; focused commands are cargo test --locked -p <package> <test_filter>, with nonzero intended test selection. Verify generated-contract parity and discoverable native qualification commands as their tooling lands. Run changed-behavior/blast-area checks during development; complete verify for exact non-Markdown commit contents. Documentation-only work requires rendered/diff inspection and git diff --check, not Rust compilation.

SugarCubes changes use its own repository environment, npm run build/build:check, strict TypeScript/Python typing and lint/format/architecture checks plus python -m tools.test run <area> [proof]. Inspect tools.test list for actual selectors; register desktop tests in its behavioral policy. Check authored-to-served web artifact parity. K1's unchanged extension requires observed served identity/functionality, not full gates over unrelated upstream work.

Extend CI native development packages deliberately: Linux GTK3/WebKitGTK 4.1/pkg-config requirements, macOS system WebKit/framework and application bundle metadata, Windows declared MSVC/WebView2 strategy. Keep the existing portable three-platform build/unit gate and native harness receipts separate. Windows Server 2022 runners do not prove Windows 11 Mica. Headless browser tests do not prove native views/input/accessibility. No ignored tests or unrun-platform success claims.

**Acceptance:** Every required native/functional/security/resource/performance receipt is reviewed and passing, with exact current artifacts and explicit observed environments. Applicable failures block acceptance. Missing platform evidence keeps that platform and full checkpoint acceptance pending; it does not retroactively weaken the supported-platform requirement.

## 11. H1 — Deliver the reproducible SugarCubes development environment

**Entry:** Q1 full acceptance. **Owner:** Integration lead and independent handoff reviewer. **Deliverable:** Runnable developer artifacts and a completed evidence ledger, not an installer.

**Implementation:** Not started. **Verification:** Pending.

- [ ] Package and qualify reproducible outside-checkout developer artifacts and publish the actual verified launch commands.

Produce locked debug/release build/launch receipts, executable/resources/licenses and explicit platform runtime requirements, including internal supervisor/desktop/guardian/reporting modes, the packaged crash-capture runtime, diagnostics storage and exact-release symbol provenance. Prove helpers exit after their last owned client/resource and that native reporting works with the browser/backend unavailable. Prove the artifact launches from outside the Rust and legacy desktop source checkouts with its packaged characters/icons and ignored configuration. Windows detects a healthy Evergreen runtime; macOS has deliberate application bundle identity/menu/resources/profile store and minimum OS; Linux has declared GTK3/WebKitGTK 4.1 (2.40+) runtime libraries. No automatic dependency acquisition, Comfy provisioning, migration or updater is added.

Record exact commands that actually work at handoff. Until implemented, proposed commands are not executable instructions. Existing proof invocation is the sugarsubstitute binary's --collapsed/--light/--reduced-motion/--locale/--duration-ms/--receipt and --scenarios/--output options. At K1 promote real startup as the default composition and retain the scripted proof as an explicit qualification mode. The final developer invocation selects validated local configuration; committed examples use placeholders and never private absolute paths.

Every acceptance entry uses: date; work ID; status; owner/reviewer; relevant exact source/lockfile/served artifacts; protocol/runtime version; command/scenario and intended test count; observed result/platform; remaining uncovered requirements; next action. Link portable receipt identifiers, with large captures in ignored permitted storage. A pending requirement cannot be hidden in an “accepted” status.

**Acceptance:** The developer artifact starts SugarSubstitute, presents the actual Comfy frontend/SugarCubes, displays real outputs in native static canvas tabs and provides the functioning host needed for further SugarCubes development. K1, I1, I2 and Q1 acceptance entries identify the exact artifacts and platforms. All unresolved requirements remain visible under their owning work items.

**S0 integrated parity and input development receipt (2026-10-11):** Graphics owner `graphics-full-17` completed all 379 character/progress/caption scenarios and 209 console scenarios at unchanged acceptance thresholds. Character maximum premultiplied channel difference is one, worst mean 0.000004364; toggle is byte-exact. Its executable SHA-256 is `040fa6657e3cb5cf1ff5d0265476a209e2ae9f8c3dfb1d46d4cb9c1b0ea9a2ef`. The isolated comparator still fails its stale Windows status text; integration uses the already qualified DirectWrite owner. Five final character files were imported with exact delivered hashes, including logical/device transform composition, 26.6 coverage and the 256-span native sampling callback boundary. Root combined pixel qualification remains required. Numerical-source attribution and original Qt/FreeType license texts are registered.

The caption regression classified the header's overlap with the expanded mascot as mascot input. The content owner now gives header controls and dragging priority, with a regression covering both disclosure states. The Windows adapter queues the native `SC_MOVE | HTCAPTION` operation and preserves owned capture semantics; four real hidden-window tests cover command delivery, capture and destination identity. Development `native-build-07` (`ac1bb0bce9ca461aac05fc44c2432d4329016d5c51b0bda12accb6e63da96b08`) enters the OS move loop and physically moves. Its second drag observer failed because whole PrintWindow RGB includes position-dependent Mica. That failed receipt is retained; replacing RGB equality requires full-frame premultiplied source equality, a lifetime change counter, and every captured pixel's valid source-over bounds. Static and animated native drag acceptance remain pending.

The owned foreground fixture activates its own standard BUTTON through actual mouse input before process creation, retains verified process identities and allows the exact child to request foreground activation. It never forces product activation. `startup-reference-foreground-probe-02` succeeds with production reference exposure 1149.106 ms, visible 1277.466 ms and interactive 1428.295 ms; both target decisions are false, and this exploratory run is not an uncontended series. The virtual-environment redirector's verified base-Python child is retained and reaped. Final paired five-run series remains required. `native-build05-input-contention-01` exercised 32 real disclosure gestures with two owned CPU workers on two logical processors: observed feedback p50 44.537 ms, p95 71.309 ms, p99 77.913 ms, including capture sampling. Worker one-core utilizations were 79.164% and 76.454%; both were stopped and reaped. These are development measurements, not later M0 latency acceptance.

The integrated raster/caption outcome passed cargo xtask verify on Windows: 99 Rust tests (host 9, desktop 14, splash 46, tooling 30), governance, formatting, strict workspace Clippy and warning-free Rustdoc. The qualification tool suite selected 78 Python tests, all passed; git diff --check passed. An initial test-build attempt used a missing configured D temporary directory and failed before running tests; creating that owned directory and rerunning the full gate passed. No macOS/Linux execution is claimed. Native audit and final current-artifact matrices remain outstanding.

**S0 console boundary and native evidence development receipt (2026-10-11):** Commit `b72e058` preserved the raster/caption outcome. The combined `native-build-08` full comparison rejected 73 of 379 frames while every named owner region passed: four rounded console-border cap pixels lay outside the console rectangle, with maximum premultiplied difference five. The renderer now clips all console-border samples to the actual device-space widget rectangle. A regression prepaints the entire destination and checks that every pixel outside the console remains unchanged across both themes and five scales. Thirty affected full-frame development comparisons pass at unchanged thresholds; complete rebuilt-artifact qualification is running.

The optional native frame audit consumes successfully presented sources without cloning, compares complete dimensions/RGBA on every transition, and retains the first source plus at most one differing predecessor. Tests reject invalid/presentation-failed sources and detect change then restoration, dimensions and final-byte alpha changes. Static physical-drag observation checks every captured RGB pixel against its exact executable's premultiplied source-over bounds, then requires the terminal audit's complete first-source equality and zero lifetime changes. This permits the observed position-dependent native material while preserving complete foreground evidence. Failure receipts retain bounded diagnostic files, owned termination/drain phases and original causes; retained tails are capped at 64 KiB per stream. Tooling drain currently collects sparse qualification output in memory and is not production log supervision.

`cargo xtask verify` passed the exact rebuilt Windows Rust sources: 104 tests (host 9, desktop 18, splash 47, tooling 30), governance, formatting, strict workspace Clippy and warning-free Rustdoc. `python -m unittest discover -s tools/splash_parity -p test_*.py` selected 96 passing tests; `git diff --check` passed. Release `native-build-09` has executable SHA-256 `0b9dcc33ed21907c583e44d42a1564702141f87775671306b0177ee09bf27094` and matching local source/build/PDB provenance.

`native-build09-static-drag-02` passed 32 actual mouse movements with 35 successfully presented frames, complete source equality and zero source transitions. All 311,364 initial pixels and every subsequent full capture satisfied source-over bounds. Move observation p95 was 5.164 ms and message-response p95 3.950 ms. Entry took 540.024 ms; native tracing placed dispatch 21 microseconds after queueing and move-loop entry 515.734 ms after command dispatch. The preceding attempt failed the one-second entry boundary. Qt uses the same native command payload; no payload change or resolved-delay claim is justified. Animated cadence, controlled-load drag and the final native matrix remain pending.

The sequential `startup-build09-rust-series-01` and `startup-build09-reference-series-01` each contain five owned real launches with the same foreground fixture, production creation flags and retained caches. All five Rust runs meet the 500/750 ms targets: visible 216.189–375.877 ms (median 267.784), interactive 351.095–544.668 ms (median 395.986). Reference visible is 1096.304–2881.517 ms (median 1144.427), interactive 1204.602–3109.494 ms (median 1250.302); its target decisions are false. First-in-series and warm repeats are identified separately; no cold-cache claim is made. Reference-owned child and launcher exits are observed. No other GUI qualification, build or graphics export ran during these series. Windows S0 acceptance remains pending; macOS/Linux and real mixed-DPI receipts remain pending.

**S0 complete-frame raster receipt (2026-10-11):** `graphics-final-build09-01` passes all 379 main full frames but rejects six of 50 locale full frames: one Japanese or Simplified Chinese glyph sample extends left of the actual label. Its 50 named status regions remain byte-exact. The text owner now clips native coverage to the QLabel widget's complete scaled half-open rectangle, preserving all interior coverage. The regression failed before the fix, then passed all 50 locale/theme/scale cases with a prepainted destination and independent unclipped interior comparison. Qt grayscale numerical attribution now includes the Windows text owner.

The root's rebuilt `native-build-10` executable SHA-256 is `3df0d7e74e8182b247d9651db78d8ab3f8e2c63b60bdd93fa1e4fa2c34cec6b5`; matching PDB, lockfile and source snapshots are retained locally. `cargo xtask verify` passes 105 Rust tests (host 9, desktop 18, splash 48, tooling 30) and all established gates. All six observed exporter processes and the exact-build numerical qualification exited zero in `graphics-final-build10-01`: all 379 main and 50 locale full frames/owners pass at unchanged limits; all 50 locale full frames and 170 main status regions are byte-exact. Main maximum premultiplied channel difference is one. Nine fresh formal pair/difference cases include all six former glyph leaks. The lead visually reviewed the Japanese collapsed and light fractional expanded flip comparisons.

`native-build09-animated-drag-01` observes 32 physical movements and 141 submitted frames over 3.207 seconds, with native frame work mean 2.812 ms. Its 31 changing captured progress samples include native material and therefore do not prove source animation cadence. The global longest submission interval is 864.111 ms; native tracing places an 850.553 ms gap between move command and entry. Producer evidence specific to the actual move loop is being added. `caption-entry-reference-01` rejects a stationary press, but inspection finds that the installed reference title bar starts movement on mouse motion; that rejection does not establish a shared delay. The initial one-delta physical probes reject entry for both applications. Qualification is being corrected to use continued bounded physical motion and to retain the original one-second deadline and owned-loop observation. All failed receipts and owned cleanup outcomes remain preserved. Windows S0 is not accepted yet.

The shared physical entry observer now queries owned GUI state and continues horizontal four-pixel movements while pending, capped at sixteen movements/64 pixels, with the original press-to-entry one-second deadline. It stops movement on actual entry and rejects late-active state, foreign motion, missing entry, mismatched cursor, oversized observation inventory and failed teardown. The integrated tooling suite selects 120 passing tests. `caption-entry-build10-continuous-01` passes actual entry in 41.477 ms after two movements and release-to-exit in 6.353 ms. The reference continuous-gesture case still rejects entry; source inspection finds an enabled stock title bar but does not prove its actual native event recipient. That unresolved reference observation is preserved separately from current Rust proof.

Six current `native-build10-*-controls-01` cases pass real native input, standard Button accessibility/Invoke/focus, pressed/cancelled close feedback, owned unoccluded compositor and Mica: dark English/Japanese/Simplified Chinese/Korean/Spanish plus light reduced-motion English. These are functional control receipts; concurrent owner development means their incidental frame metrics are not an uncontended performance series. Final static/animated physical drag, idle visibility states and current-artifact timing remain required before development-platform acceptance.

**S0 bounded moving-source and final Windows receipt (2026-10-11):** Commit `d502b15` delivers the widget clipping and complete static-source qualification after 105 Rust and 120 tooling tests. The subsequent optional source audit retains at most 512 movement samples and two original source buffers. Each sample follows successful native/control painting bracketed by actual owned `GUI_INMOVESIZE` observations; complete-source revision, lifetime sequence and elapsed submission time are retained. Native/allocation failures preserve their causes; overflow remains explicit. Tests cover outside-loop changes, full-source changes/restoration, omitted samples and foreign native movement. The exact runtime sources pass `cargo xtask verify`: 109 Rust tests (host 9, desktop 22, splash 48, tooling 30), governance, formatting, strict Clippy and warning-free Rustdoc.

Frozen `native-build-11` executable SHA-256 is `0451f272a1038eda3bbe6fc951bc57c5781adb56c51dbf7c03bd9bbbb4aa67bc`; matching PDB and source/lock/build provenance are retained. `graphics-final-build11-01` passes all 429 complete frames and owners at unchanged budgets, including 50 byte-exact locale frames. All six exporter exits and the numerical checker exit are observed. The 25 splash source fingerprints and nine representative candidate PNGs match build10; the nine previously reviewed formal pairs/differences are reused with exact image identities.

Six `native-build11-*-controls-01` cases pass actual controls, accessibility, close/cancel feedback and Mica in every supported locale and light reduced-motion English. Five uncontended `startup-build11-rust-series-01` launches all meet the visible/interactive targets: visible 214.136–450.123 ms, interactive 331.364–556.784 ms. Caches are retained and first-in-series is identified separately; the earlier equivalent five-run production reference series remains recorded above. Three `native-build11-static-{visible,minimized,hidden}-01` cases each observe a complete 30-second interval with zero added CPU ticks, 13 unchanged GDI objects and restored paint. The actual monitor receipt contains one 96-DPI monitor; real mixed-DPI movement remains unavailable/pending, independently of the five-scale numerical comparisons.

The first current static and animated drag receipts reject an origin mismatch: queued horizontal entry movement satisfied an early two-pixel position tolerance before the final cursor/window anchor was established. The harness now performs two recorded, bounded diagonal calibration movements. Fresh vertical translation distinguishes the first seed from queued horizontal entry; the second must match its complete eight-pixel translation at the original two-pixel tolerance. Actual cursor identity and owned loop state are checked throughout, with original per-phase and overall deadlines. Calibration is excluded from the 32 measured movements. Rejection tests preserve old-entry, deadline, cursor, loop-loss, unsafe-area and mismatched-second-probe failures. The complete tooling suite selects 145 passing tests.

`native-build11-static-drag-02` and `native-build11-static-drag-contention-01` pass all 32 actual movements and complete source-over checks, with respectively 36 and 35 presented frames and zero source changes. Observed movement p95 is 6.604/5.084 ms and message response p95 is 4.542/4.103 ms. `native-build11-animated-drag-02` retains 139 consecutive in-loop submissions and 138 source transitions over 2034.154 ms; intervals p50/p95/p99/max are 15.389/29.045/32.724/32.975 ms. Its contention counterpart retains 135 submissions/134 transitions over 2029.612 ms, interval p95 28.208 ms, lifetime maximum 51.705 ms. Both have zero omitted samples; only native source evidence grants animation acceptance. Captured RGB activity is diagnostic. These intervals cover in-loop submission pairs, not compositor frame age or complete entry/exit gaps.

Controlled runs use two owned integer-load workers and explicit two-logical-CPU affinity, with observed worker utilization and terminal/reap receipts. They do not claim full CPU saturation. The 32 disclosure input receipts measure capture-inclusive visible feedback p50/p95/p99 of 34.382/61.906/75.425 ms uncontended and 33.675/63.140/69.017 ms under controlled load. The later M0 input p95 50 ms and submission p95 25 ms targets are not established by these S0 measurements. Off-thread preparation and bounded delivery remain M0 work.

**Windows development S0 acceptance:** The independent native reviewer verified the exact frozen executable, complete static-source/composition evidence, consecutive moving-source activity, actual control/accessibility/material results, five target-passing startup runs and the three idle visibility intervals. No blocker remains for this Windows 11 development setup at 96 DPI. Native submission inventories include expose/input as well as scheduled animation; their counts do not establish a universal 60-fps cap. The root's final exact-content `cargo xtask verify`, 145-test Python suite and `git diff --check` all pass. S0's reusable development boundary is accepted and M0 may begin. Actual mixed-DPI, macOS and Linux X11/Wayland qualification remain pending; no compositor frame-age or move-loop edge-gap measurement is claimed.

**M0 implementation entry:** Commit `cabcf4e` preserves the accepted development S0 boundary. Execution, pure contracts and owner-export enforcement use fresh exclusive worktrees under `E:\devprojects`, rooted at that commit. Application owns semantic scope/job/surface generations; execution owns admission, factories, actual-stop observation, joins and delivery. Generic typed submission ports retain private task erasure, with no public runtime handles. Lifecycle work may consume unused ordinary capacity while ordinary work cannot consume reserved capacity. Delivery credits remain held until typed result transfer; cloned result-memory leases retain the original byte charge until the last guard drops. Scope/job abandonment requests cancellation while execution retains the real lifetime. Explicit shutdown reports unresolved work and retains ownership for retry; final executor destruction belongs after native UI teardown and joins retained workers.

The first implemented contracts behavior is canonical localization: all five existing locale records and their coverage/fallback test move unchanged from splash to the domain contracts owner. Host and renderer callers consume that owner directly; the existing public splash text exports remain supported. Architecture/testing policy registers the new owner. Initial focused tests pass; lockfile regeneration initially refreshed unrelated transitive entries, which were restored. Offline minimal resolution now adds only the contracts package and its two workspace dependency edges, preserving all existing dependency versions. No M0 execution, native responsiveness or later supervision acceptance is claimed yet.


**M0 contract/readiness development receipt (2026-10-11):** Contracts now own the unchanged English, Simplified Chinese, Japanese, Korean and Spanish locale registry. Splash and host callers consume that owner; the existing public splash localization exports remain supported. The domain execution boundary has owned scopes/jobs, typed admission and terminal outcomes, scheduler-neutral clock/cancellation waits, shared retained-byte leases and bounded native completion metadata. One release returns each byte charge only after its final lease; terminal transfer independently acknowledges delivery capacity. Factories execute only after admission on an execution worker. Cancellation requests remain separate from actual stop. Panic text copies into a fresh bounded UTF-8 allocation rather than retaining an arbitrarily large input capacity. Eleven execution-contract tests pass.

Accepted scope and job IDs are independently increasing within one executor lifetime. The adapter retains bounded high-water marks instead of animation-rate retired-ID sets; rejected admission does not advance them. Sixty-four live/retained scopes cap metadata. Interactive CPU jobs require a surface replacement key; other classes reject one. Queue/ordinary/lifecycle, completion and memory reservations remain atomic, with a new replacement requiring its own terminal credit before retiring the old pending job. Private adapter implementation and rejection/lifecycle tests are still underway.

The implemented frontend-readiness protocol accepts schema one, checks the 4096-byte bound before JSON parsing, rejects unknown fields/versions and nonpositive identities, and requires usable frontend graph/canvas/loading plus completed SugarCubes registration/readiness. Observed version labels and diagnostic detail have separate bounds. Typed failure detail is preserved without becoming product copy; JSON display reports category/location without echoing untrusted field names. The immutable receipt carries launch and navigation identities, grants no native authority, and requires desktop-origin validation before parsing. Persistence governance registers its exact sources with reject-only schema compatibility. Seven protocol tests and five application tests cover complete/incomplete evidence, malformed/oversized data, stale or future navigation, foreign launches, terminal duplicates, original failures, reload and permanent cancellation. This proves protocol/application rules, not an actual webview or Comfy readiness.

Actual lead commands are `cargo test --locked -p sugarsubstitute-contracts -p sugarsubstitute-application` (19 + 5 passing tests), focused strict all-target/all-feature Clippy for contracts/application/host/splash, formatting and governance. The integrated owner-export gate consumes canonical dependency rename metadata; the temporary unused-field warning is resolved. Full exact-content repository verification is the next commit prerequisite. Existing external dependency versions remain unchanged; lock resolution added only implemented workspace packages/edges. Fresh execution, governance and CPU worktrees are under `E:\devprojects`; all test temporary storage and receipts are configured on the permitted artifact drive.


**M0 owner-export governance development receipt (2026-10-11):** The integrated gate inspects declared public execution boundaries while retaining private runtime implementations. Rejection fixtures cover Cargo renames, lexical/type aliases, grouped/glob/nested re-exports, public fields/signatures/generic defaults, trait/union/reference and explicit associated-type projections, function-pointer initializers, direct and conditionally exported macros, local recursive macro initializers, emitted helper signatures and platform module-path alternatives. Review found and fixed missed default trait constants, public union/local-trait projections, hidden type macros and conditional exports; valid private generic implementations and tracing field names remain accepted. Thirty-seven tooling tests pass in the canonical checkout. Static declaration analysis does not establish lifecycle correctness or arbitrary behavior-body/external-macro inference; execution tests and source review retain those responsibilities. The absence of a registered canonical execution adapter still forbids all consumer scheduling.

Independent review accepted the bounded readiness/navigation and locale move. Six additional malformed-input probes rejected duplicate IDs/status/evidence, floating-point identity, missing evidence and unknown failure fields; the equivalent regression now runs in the protocol owner. Actual command `python -m unittest discover -s tools/splash_parity -p "test_*.py" -q` selected 145 passing qualification-tool tests. Native graphics/readiness claims remain the existing S0 receipts and pending production integration, respectively.

**M0 first boundary verification receipt (2026-10-11):** Canonical exact runtime/tooling contents passed cargo xtask verify on Windows with Rust 1.93.1-x86_64-pc-windows-msvc: governance, formatting, strict workspace/all-target/all-feature Clippy, 139 Rust tests (host 9, contracts 19, application 5, desktop 22, splash 47, tooling 37), no ignored tests, and warning-free Rustdoc. Cargo.lock SHA-256: f55692b8b594a77203757bae44b50cc15b72ca1ba41a68cb23c9195158468b97. Qualification-tool suite: 145 passing tests. New crates have real protocol/admission behavior and registered test ownership. The execution runtime remains in its implementation worktree and is not yet registered in the canonical checkout; native M0 targets, startup configuration and M1/K1 remain pending. This coherent boundary outcome is ready for the next authorized interval commit; it is not M0 acceptance.
