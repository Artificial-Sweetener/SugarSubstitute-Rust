// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its file-drag.schema.json; the workspace
// compiles that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/FileDragProtocol.generated.rs");

use super::{PROTOCOL, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(message: &super::FileDragHostMessage, session: &str) -> FileDragHostMessage {
    let (protocol, version, session) = (PROTOCOL.to_owned(), VERSION, session.to_owned());
    match message {
        super::FileDragHostMessage::Entered { names } => {
            FileDragHostMessage::Entered(FileDragEnteredMessage {
                protocol,
                version,
                session,
                r#type: "entered".into(),
                files: names.iter().map(|name| FileDragFile { name: name.clone() }).collect(),
            })
        }
        super::FileDragHostMessage::Left => FileDragHostMessage::Left(FileDragLeftMessage {
            protocol,
            version,
            session,
            r#type: "left".into(),
        }),
    }
}
