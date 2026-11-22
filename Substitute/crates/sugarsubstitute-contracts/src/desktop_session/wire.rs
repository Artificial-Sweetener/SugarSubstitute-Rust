// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its session.schema.json; the workspace compiles that
// single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/SessionProtocol.generated.rs");

/// Version 2 of the checkpoint, generated from session-checkpoint-v2.schema.json: version 1 with
/// each entry's view naming where its workflow's masonry is scrolled. Only the changed values are
/// defined here; every other one is version 1's own, so checkpoints and session messages keep
/// sharing one output, file and scope type.
pub mod v2 {
    #[allow(clippy::wildcard_imports)]
    use super::*;
    include!(
        "../../../../../Sugar/integrations/contracts/SessionCheckpointV2Protocol.generated.rs"
    );
}
