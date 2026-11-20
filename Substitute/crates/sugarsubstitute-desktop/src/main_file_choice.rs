// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold one main window's page request for a file dialog until the application shows it, and
//! refuse a second while the first is waiting or showing. This owner holds no native resources.

use sugarsubstitute_contracts::host_file_choice::{FileChoiceClientMessage, FileChoiceRequestName};

/// The one request a window serves at a time.
#[derive(Debug, Default)]
pub(crate) struct ChoiceDesk {
    waiting: Option<FileChoiceClientMessage>,
    showing: bool,
}

impl ChoiceDesk {
    /// Accept `message` for the application to show, or name the request it refuses because a
    /// dialog is already waiting or showing.
    pub(crate) fn ask(
        &mut self,
        message: FileChoiceClientMessage,
    ) -> Result<(), FileChoiceRequestName> {
        if self.waiting.is_some() || self.showing {
            return Err(message.request);
        }
        self.waiting = Some(message);
        Ok(())
    }

    /// Take the waiting request to show it; it counts as showing until [`Self::finished`].
    pub(crate) fn take(&mut self) -> Option<FileChoiceClientMessage> {
        let request = self.waiting.take();
        self.showing |= request.is_some();
        request
    }

    /// The shown request has been answered; another may be asked.
    pub(crate) fn finished(&mut self) {
        self.showing = false;
    }
}

/// The chosen file's name alone, and its path below `folder` with forward slashes; the path
/// is empty when the choice lies outside `folder`. Letter case is ignored as Windows does.
pub(crate) fn place(folder: &std::path::Path, chosen: &std::path::Path) -> (String, String) {
    let text = |part: std::path::Component<'_>| part.as_os_str().to_string_lossy().into_owned();
    let name = chosen.file_name().map(|name| name.to_string_lossy().into_owned());
    let (root, path): (Vec<String>, Vec<String>) =
        (folder.components().map(text).collect(), chosen.components().map(text).collect());
    let below = path.len() > root.len()
        && root.iter().zip(&path).all(|(root, part)| root.to_lowercase() == part.to_lowercase());
    let relative = if below { path[root.len()..].join("/") } else { String::new() };
    (name.unwrap_or_default(), relative)
}

/// The folder a dialog starts in: `start`, a forward-slashed path below `folder`, or `folder`
/// itself when `start` is empty or names anything but plain folder names.
pub(crate) fn starting_folder(folder: &std::path::Path, start: &str) -> std::path::PathBuf {
    let plain = |part: &&str| {
        !part.is_empty()
            && *part != "."
            && *part != ".."
            && !part.contains(['\\', ':'])
            && !part.chars().any(char::is_control)
    };
    let parts: Vec<&str> = start.split('/').collect();
    if start.is_empty() || !parts.iter().all(plain) {
        return folder.to_path_buf();
    }
    parts.iter().fold(folder.to_path_buf(), |path, part| path.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::host_file_choice::FileChoiceClientMessage;

    fn request(name: &str) -> Result<FileChoiceClientMessage, Box<dyn std::error::Error>> {
        Ok(FileChoiceClientMessage::decode(
            format!(
                r#"{{"protocol":"sugarcubes.file-choice","version":1,"session":"session-12345678","type":"open","request":"{name}","folder":"C:\\projects","start":"","extension":"sugar"}}"#
            )
            .as_bytes(),
        )?)
    }

    #[cfg(windows)]
    #[test]
    fn a_choice_is_placed_below_the_starting_folder_or_nowhere() {
        use std::path::Path;
        let folder = Path::new(r"E:\ComfyUI\user\default\projects");
        for (chosen, name, relative) in [
            (r"E:\ComfyUI\user\default\projects\Demo\Demo.sugar", "Demo.sugar", "Demo/Demo.sugar"),
            (r"e:\comfyui\USER\default\Projects\a.sugar", "a.sugar", "a.sugar"),
            (r"E:\ComfyUI\user\default\projects-old\a.sugar", "a.sugar", ""),
            (r"E:\ComfyUI\user\default\a.sugar", "a.sugar", ""),
            (r"D:\elsewhere\projects\a.sugar", "a.sugar", ""),
            (r"E:\ComfyUI\user\default\projects", "projects", ""),
        ] {
            assert_eq!(place(folder, Path::new(chosen)), (name.to_owned(), relative.to_owned()));
        }
    }

    #[test]
    fn a_dialog_starts_below_its_folder_and_never_outside_it() {
        use std::path::{Path, PathBuf};
        let folder = Path::new("projects");
        for (start, expected) in [
            ("", "projects"),
            ("Demo", "projects/Demo"),
            ("Demo/takes", "projects/Demo/takes"),
            ("..", "projects"),
            ("Demo/../..", "projects"),
            ("/Demo", "projects"),
            ("C:/Windows", "projects"),
            ("Demo\\..\\..", "projects"),
            ("Demo//x", "projects"),
        ] {
            let expected: PathBuf = expected.split('/').collect();
            assert_eq!(starting_folder(folder, start), expected, "{start}");
        }
    }

    #[test]
    fn one_request_is_served_at_a_time_from_asking_to_its_answer()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut desk = ChoiceDesk::default();
        assert!(desk.take().is_none());
        assert!(desk.ask(request("first")?).is_ok());
        let refused = desk.ask(request("second")?).err().ok_or("a waiting request was replaced")?;
        assert_eq!(refused.as_str(), "second");
        assert_eq!(desk.take().ok_or("nothing to show")?.request.as_str(), "first");
        assert!(desk.take().is_none(), "a request is shown once");
        assert!(desk.ask(request("third")?).is_err(), "the dialog is still showing");
        desk.finished();
        assert!(desk.ask(request("fourth")?).is_ok());
        Ok(())
    }
}
