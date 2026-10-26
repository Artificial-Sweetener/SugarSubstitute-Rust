# AGENTS.md

## Mission and repository scope

SugarSubstitute is the Rust desktop host for ComfyUI frontend, SugarCubes, and
the native canvas system. The product name is SugarSubstitute; the checkout
directory name does not change application or installation identity.

This file defines engineering guardrails, not feature specifications,
implementation plans, or workstation configuration.

## Authoritative owners

- ComfyUI frontend owns its workflow graph, workflow tabs, node controls, and
  existing frontend lifecycle. ComfyUI owns inference execution.
- SugarCubes owns the web interface and its desktop presentation: canvas tab
  controls, panels, menus, toolbars, and host-aware styling run inside ComfyUI
  frontend. Consume its supported APIs rather than copying its UI here.
- SugarSubstitute owns desktop windowing, startup and recovery, installation,
  updates, connection/process lifecycle, OS services, workspace document
  lifecycle, and workflow/canvas/result coordination. Its native UI serves
  window chrome, OS dialogs, and situations where the web interface is unavailable.
- Native canvas owners belong to this workspace. `sugarsubstitute-image-ops`
  owns image-operation graphs, pyramid/resampling semantics and tiled evaluation.
  `sugarsubstitute-canvas` owns document snapshots, reversible layer commands,
  viewport transforms and damage planning. `sugarsubstitute-canvas-render` owns
  prepared canvas frames and retained CPU/GPU presentation. Native windows and input
  remain with the desktop adapter; consume each owner's public APIs.
- SugarPrompt integration belongs to SugarCubes. Do not introduce a second
  editor integration in the desktop host.

## Architecture and change discipline

- Assign one authoritative owner per concern. Before changing a component,
  identify its state, dependency direction, lifecycle, public contract, and tests.
- Use cohesive Rust modules, explicit owned types, traits at real adapter
  boundaries, and typed commands/events/errors. Do not impose class hierarchies
  or abstract traits where concrete types communicate the design more clearly.
- Domain contracts remain independent of windowing, webviews, network clients,
  filesystem implementations, and canvas host handles. Application services
  coordinate use cases; adapters implement external boundaries; the host composes
  them. Runtime crates must never depend on repository tooling.
- Add crates when a real boundary requires independent ownership. Register every
  workspace crate in architecture and testing policy in the same change.
- Do not add behavior to mixed-responsibility modules. Characterize behavior,
  extract authoritative owners, update every caller, and remove replaced code.
  Avoid forwarding shims, generic dumping grounds, and duplicate internal rules.
- Preserve supported public behavior and persisted data. Internal compatibility
  layers are not a goal. Public contract changes require coordinated consumers,
  explicit compatibility decisions, and contract tests.
- Prefer complete, verified vertical slices. Resolve implementation questions by
  inspecting the owners before presenting a plan; leave only genuine product or
  external-dependency decisions for the maintainer.
- Do not start feature implementation during an exploratory question. Implement
  only the scope explicitly requested by the maintainer.

## Execution ownership

- One dedicated execution adapter owns async runtimes, task spawning, CPU worker
  pools, queue admission, cancellation/deadlines, task supervision and shutdown.
  Application services own use-case decisions and submit work through its typed
  ports. Do not create ad hoc threads, executors, pools or detached tasks.
- Register the implemented adapter in `governance/execution/policy.toml`. Until
  that owner exists, scheduling primitives are forbidden in authored Rust.
  Runtime handles and raw spawn APIs remain private to that adapter; consumers
  receive owned job lifetimes and typed results, not scheduler escape hatches.
- Every job belongs to an application/task scope with explicit cancellation,
  bounded concurrency and queue/memory admission, an observed terminal outcome,
  and bounded shutdown. Reject or coalesce overload explicitly. Dropping a
  future or cancellation token is not proof that blocking/native work stopped.
- Native event-loop pumping and thread-affine window/webview operations remain
  with the desktop adapter. Render preparation uses the execution owner when
  offloaded; presentation consumes immutable prepared results on its required
  thread. Never wait for a job on the UI thread or publish stale-generation work.
- Tests use the same execution boundary and deterministic clocks/boundary fakes.
  Test modules, platform conditionals and repository tooling are not exemptions.
- The execution gate checks known scheduler dependency identities and source
  access, including aliases and macro tokens. Review new scheduling libraries,
  native entry points and exported execution contracts, and extend the gate with
  rejection fixtures in the same change. Source inspection does not prove task
  lifecycle correctness; behavior tests and native qualification must do that.

## Host and native surface contracts

- Shared native widgets belong to `sugarsubstitute-ui`. Surfaces own content and
  layout; the desktop adapter owns OS windows, event loops and native peer lifetimes.
  `governance/ui/policy.toml` registers toolkit dependencies and exact reviewed
  platform/composition sources. The UI gate rejects unregistered construction,
  changed boundary sources and stale records, including aliases and macro uses.
  Add reusable widgets to the shared owner rather than drawing ad-hoc controls.
- The desktop embeds the actual ComfyUI frontend with SugarCubes installed.
  Preserve ComfyUI's supported extension lifecycle and server behavior.
- Version public host contracts and document their compatibility requirements.
  Presentation state must not grant native privileges.
- Native IPC uses specific typed operations, request identities, bounded payloads,
  actionable errors, cancellation, and revision checks where ordering matters.
  Verify message origin and parameters. Other extensions share the frontend's
  execution context; an injected JavaScript object is not a trust boundary.
- Give native handles, callbacks, workers, and buffers explicit ownership,
  thread affinity, cancellation, and teardown. Keep blocking work off the UI thread.
- Isolate platform-specific behavior in adapters with explicit contracts.
  Keep rendering policy with its authoritative owner.
- Never enter a native modal dialog or another application-event-loop pump from an
  application event callback. Submit a request to its desktop owner and receive
  the result later. Modal owners dispatch on the required UI thread between
  application callbacks, retain the original peer through actual modal return,
  and handle cancellation and teardown explicitly. The UI gate isolates known
  modal APIs in exact reviewed sources and rejects mixing them with window or
  event-loop construction. Native OS-managed move/resize dispatch remains with
  its reviewed desktop adapter.

## Rust toolchain and dependency policy

- `rust-toolchain.toml` owns the Rust toolchain pin.
  Edition 2024, MSRV, shared dependency versions, profiles, and lints live in the
  workspace `Cargo.toml`. Every member inherits workspace metadata and lints.
- Run commands from the repository root. Check `rustup show active-toolchain`
  when ambient overrides could bypass the pin. Do not use global stable/nightly
  overrides, upgrade the toolchain, or change build configurations incidentally.
- Commit `Cargo.lock`. Use `--locked` for checks and builds after deliberate lockfile
  generation. Shared dependencies use explicit versions and reviewed features.
- Add only dependencies needed by implemented behavior. SugarCubes owns its
  frontend build.
- Preserve established build-tree toolchains, SDKs, architectures, and dependency
  roots. Use a separate build directory for a deliberate configuration change.
- Keep machine-specific paths and configuration in local, ignored files.
- Avoid unsafe code. Platform FFI exceptions require a narrow adapter, documented
  safety/lifetime invariants, a scoped lint allowance, and native tests. Never
  relax the workspace unsafe lint to accommodate one adapter.
- Do not silently ignore results, use panic/unwrap/expect for fallible runtime
  work, or leave todo/unimplemented paths. Model errors at the authoritative owner
  and preserve causes when crossing boundaries.

## Executable governance

- `cargo xtask governance` checks architecture ownership, dependency direction,
  source structure, debt/waivers, test ownership, persisted formats, cache entries,
  execution ownership and vendored-file registration. It fails on unknown schemas
  and stale records.
- `governance/architecture/policy.toml` owns authored Rust source roots and crate
  layers. Structural thresholds are 350 soft and 500 hard nonblank code lines,
  excluding standalone comments. Size is an ownership alarm, not proof of cohesion.
- `debt.toml` records current mixed ownership, exact source fingerprints,
  responsibilities, tracking issue, next extraction, and review expiry.
- `waivers.toml` permits a source-reviewed cohesive structural exception or a
  debt-linked remediation with an exact current limit and a lower next limit.
  Fingerprints are SHA-256 over normalized source. Remove resolved/unused records.
  Do not raise limits or extend expiry solely to pass a gate.
- A structural waiver requires a file-specific explanation of the authoritative
  contract and why extraction would split its invariants. Mixed concerns require
  debt and extraction rather than a structural waiver. The checker verifies
  record integrity; human review establishes cohesion.
- `governance/testing/policy.toml` maps each crate to its behavioral test owner and
  a nonzero test inventory. Rust test discovery rejects ignored tests. Any
  platform/serial exception requires explicit policy support and rejection tests
  before use; do not conceal failures with skips or unconditional serialization.
- Governance tools are production-quality code. Add deliberately invalid fixtures
  proving rejection whenever a rule is added or changed. A policy file without
  enforcement must not be described as an enforced gate.

## Persistence, cache, and upgrade safety

- Register every implemented persisted format and cross-version host protocol in
  `governance/persistence/catalog.toml` with its owner, source fingerprint,
  classification, preservation, current/accepted versions, and migration policy.
- Advance incompatible schemas and provide contiguous supported forward
  migrations. Preserve original bytes, validate new data before activation, and
  make migrations idempotent, restart-safe, and recoverable.
- Preserve sessions, settings, and authored documents during installation changes.
  Keep desktop installation ownership separate from ComfyUI ownership. Preserve
  managed/attached ComfyUI environments, models, custom nodes, outputs, credentials,
  authored projects, and external Python bindings.
- Cleanup requires verified installation ownership and resolved paths, including
  reparse points/symlinks. Unknown files or cache-root content remain intact.
- Stage upgrades, preserve recovery copies, validate the new application, activate
  atomically, and retire verified desktop payloads with restart-safe cleanup.
  Preserve release authenticity, rollback protection, and activation health checks.
- Register persistent caches in `governance/cache/catalog.toml`, with a unique
  namespace, schema/epoch, semantic compatibility inputs, freshness, retention,
  and recovery. Obtain storage through the cache owner. Preferences, trust,
  sessions, projects, and outputs are authoritative data, never disposable cache.
- Corrupt/incompatible cache entries are observable misses. Invalidate only the
  affected owner, retain compatible generations, and keep cleanup unable to reach
  authoritative state. Process-only memoization stays with its runtime owner.

## Localization, documentation, and third-party material

- Route SugarSubstitute-owned user-facing text, including launcher/installer text,
  through an explicit localization owner. Developer tooling diagnostics are not
  product copy. Preserve user-authored and cube-authored text exactly.
- Maintain one authoritative supported-locale registry. English is canonical and
  fallback. All release-enabled locales require complete owned-surface coverage;
  write translations directly without external aids.
- Consume ComfyUI/SugarCubes-owned text through those owners. Do not maintain a
  parallel node corpus. Preserve Unicode, IME, focus, and layout behavior.
- Write Rustdoc for new/changed modules, types, functions, and methods, explaining
  intent and non-obvious constraints. Public boundaries document errors, units,
  coordinates, ownership, cancellation, and thread affinity as applicable.
- Inline comments explain external constraints or invariants. Names communicate
  mechanics. Documentation describes implemented behavior directly.
- Do not create design docs, ADRs, notes, or README variants unless requested.
  Put routine context in types, Rustdoc, tests, and authoritative policy.
- Register vendored source/assets/derivatives in `third_party/manifest.toml`, with
  source URL, license, license file, and exact runtime files. Store license text in
  `third_party/licenses/`; add NOTICE when material is actually vendored. Cargo
  dependency provenance is owned by manifests and the committed lockfile.
- First-party Rust source carries GPL-3.0-or-later SPDX and copyright headers.

## Observability and security

- Runtime diagnostics use structured logging with operation and relevant document,
  workflow, request, execution, or connection identities. Never use stdout printing
  as a runtime logging system. Tooling may print developer diagnostics.
- Preserve unexpected error causes. Do not swallow failures or ignore cancellation.
  Never log credentials, tokens, or unnecessary sensitive paths.
- Use argument-list subprocesses, bounded network requests, explicit process
  identity, and owned-process shutdown. Do not stop attached/user-owned processes.
- Treat package trust, updates, custom-node installation, local assets, native IPC,
  filesystem operations, and credential migration as security-sensitive. Validate
  external input and fail closed at uncertain trust/ownership boundaries.

## Tests and verification

- Test observable behavior at its owner. Add regression/characterization protection
  before behavior-critical restructuring and verify every behavior change.
- Tests are deterministic, isolated, parallel-safe, and portable by default. Use
  owned temporary directories and synchronization, not arbitrary sleeps, retries,
  environment races, fixed local paths, or collection order.
- Fake external boundaries rather than the behavior under test. Native lifecycle,
  focus, input, IME, bounds, and teardown require real host qualification.
  Verify rendering policy and actual device selection independently.
- Unit test functions live with their owner; public contract/integration tests live
  under the owning crate's `tests/` directory. Tooling tests live with the tool.
- `cargo xtask check`: governance, formatting, and strict Clippy/type checks.
- `cargo xtask verify`: the same checks plus workspace tests and warning-free Rustdoc.
- Focused tests: `cargo test --locked -p <package> <test_filter>`. Verify that the
  intended tests actually ran. Use focused checks during normal implementation.
- Before ending an implementation turn, run changed-behavior/blast-area tests,
  governance, formatting, and strict lint/type checks for affected crates. Before
  a non-documentation commit, run `cargo xtask verify` for the exact contents.
- The pre-commit configuration and three-platform CI invoke the same verify command
  for changes containing non-Markdown files.
  Hooks are opt-in via pre-commit; repository setup does not install global hooks.
- Documentation-only changes modify Markdown and nothing else. Inspect the diff,
  rendered content, and `git diff --check`; they do not require Rust compilation.
- Reuse successful exact-content checks; repeat affected gates after changes.
  Applicable failures block acceptance. Do not weaken checks or budgets to pass.
- Report commands, selected tests, results, and platforms actually verified. CI
  configuration is not evidence of successful macOS/Linux execution.

## Git and maintainer authority

- Preserve unrelated changes. Do not reset, clean, stage, or overwrite others' work.
- Commit only when explicitly asked. Do not create a remote repository, push,
  publish, change global Git configuration, or install hooks without authorization.
- Each commit delivers one coherent independently releasable outcome, with its
  supporting tests and cleanup. Use `type(scope): changelog-ready outcome`.
- `feat`, `fix`, and `perf` describe audience-facing behavior. Use `refactor`,
  `test`, `docs`, `build`, `ci`, or `chore` for cohesive internal changes.
- Mark public breaking changes with `!` and describe compatibility/migration.
  Avoid WIP, checkpoint, mixed-outcome, and implementation-diary commits.
- Maintainer instructions override this file. Explain a genuine unresolved
  conflict before requesting direction; do not invent approval requirements.
