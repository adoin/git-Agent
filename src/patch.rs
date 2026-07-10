use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CreatePatchTab {
    #[default]
    Worktree,
    History,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchSelectionGesture {
    Plain,
    Toggle,
    ReplaceRange,
    AddRange,
}

pub(crate) fn selection_gesture(ctrl: bool, shift: bool) -> PatchSelectionGesture {
    match (ctrl, shift) {
        (false, false) => PatchSelectionGesture::Plain,
        (true, false) => PatchSelectionGesture::Toggle,
        (false, true) => PatchSelectionGesture::ReplaceRange,
        (true, true) => PatchSelectionGesture::AddRange,
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommitPatchSelection {
    ordered: Vec<String>,
    selected: HashSet<String>,
    anchor: Option<String>,
}

impl CommitPatchSelection {
    pub(crate) fn apply(&mut self, visible: &[String], hash: &str, gesture: PatchSelectionGesture) {
        match gesture {
            PatchSelectionGesture::Plain => {
                self.selected.clear();
                self.ordered.clear();
                self.insert(hash.to_owned());
            }
            PatchSelectionGesture::Toggle => {
                self.toggle(hash);
            }
            PatchSelectionGesture::ReplaceRange | PatchSelectionGesture::AddRange => {
                let range = self.visible_range(visible, hash);
                if gesture == PatchSelectionGesture::ReplaceRange {
                    self.selected.clear();
                    self.ordered.clear();
                }
                for item in range {
                    self.selected.insert(item);
                }
                self.normalize_visible_order(visible);
            }
        }
        self.anchor = Some(hash.to_owned());
    }

    pub(crate) fn toggle_visible(&mut self, visible: &[String]) {
        if !visible.is_empty() && visible.iter().all(|hash| self.selected.contains(hash)) {
            let visible = visible.iter().collect::<HashSet<_>>();
            self.selected.retain(|hash| !visible.contains(hash));
            self.ordered.retain(|hash| self.selected.contains(hash));
        } else {
            for hash in visible {
                self.insert(hash.clone());
            }
        }
    }

    pub(crate) fn ordered(&self) -> Vec<String> {
        self.ordered.clone()
    }

    pub(crate) fn retain_available(&mut self, available: &HashSet<String>) {
        self.selected.retain(|hash| available.contains(hash));
        self.ordered.retain(|hash| self.selected.contains(hash));
        if self
            .anchor
            .as_ref()
            .is_some_and(|hash| !available.contains(hash))
        {
            self.anchor = None;
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    fn insert(&mut self, hash: String) {
        if self.selected.insert(hash.clone()) {
            self.ordered.push(hash);
        }
    }

    fn toggle(&mut self, hash: &str) {
        if self.selected.remove(hash) {
            self.ordered.retain(|selected| selected != hash);
        } else {
            self.insert(hash.to_owned());
        }
    }

    fn visible_range(&self, visible: &[String], hash: &str) -> Vec<String> {
        let clicked = visible.iter().position(|candidate| candidate == hash);
        let anchor = self
            .anchor
            .as_ref()
            .and_then(|anchor| visible.iter().position(|candidate| candidate == anchor));
        let (Some(clicked), Some(anchor)) = (clicked, anchor) else {
            return vec![hash.to_owned()];
        };
        let start = clicked.min(anchor);
        let end = clicked.max(anchor);
        visible[start..=end].to_vec()
    }

    fn normalize_visible_order(&mut self, visible: &[String]) {
        let visible_set = visible.iter().collect::<HashSet<_>>();
        let hidden = self
            .ordered
            .iter()
            .filter(|hash| self.selected.contains(*hash) && !visible_set.contains(*hash))
            .cloned()
            .collect::<Vec<_>>();
        self.ordered = visible
            .iter()
            .filter(|hash| self.selected.contains(*hash))
            .cloned()
            .chain(hidden)
            .collect();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchPathError {
    Empty,
    Directory,
    MissingParent,
}

pub(crate) fn validate_patch_output_path(path: &Path) -> Result<(), PatchPathError> {
    if path.as_os_str().is_empty() {
        return Err(PatchPathError::Empty);
    }
    if path.is_dir() {
        return Err(PatchPathError::Directory);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if parent.is_some_and(|parent| !parent.is_dir()) {
        return Err(PatchPathError::MissingParent);
    }
    Ok(())
}

pub(crate) fn numbered_patch_paths(base: &Path, count: usize) -> Vec<PathBuf> {
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("patch");
    let extension = base
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    (1..=count)
        .map(|index| parent.join(format!("{stem}-{index:04}{extension}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn hashes() -> Vec<String> {
        ["a", "b", "c", "d"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn plain_toggle_and_range_gestures_share_one_selection() {
        let visible = hashes();
        let mut selection = CommitPatchSelection::default();

        selection.apply(&visible, "b", PatchSelectionGesture::Plain);
        assert_eq!(selection.ordered(), vec!["b"]);

        selection.apply(&visible, "d", PatchSelectionGesture::Toggle);
        assert_eq!(selection.ordered(), vec!["b", "d"]);

        selection.apply(&visible, "c", PatchSelectionGesture::ReplaceRange);
        assert_eq!(selection.ordered(), vec!["c", "d"]);

        selection.apply(&visible, "a", PatchSelectionGesture::AddRange);
        assert_eq!(selection.ordered(), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn filtering_retains_hidden_selection() {
        let mut selection = CommitPatchSelection::default();
        selection.apply(&hashes(), "b", PatchSelectionGesture::Plain);
        selection.apply(&hashes(), "d", PatchSelectionGesture::Toggle);
        selection.toggle_visible(&["a".into(), "c".into()]);
        assert_eq!(selection.ordered(), vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn numbered_paths_keep_basename_extension_and_order() {
        assert_eq!(
            numbered_patch_paths(Path::new("D:/out/review.diff"), 2),
            vec![
                Path::new("D:/out/review-0001.diff").to_path_buf(),
                Path::new("D:/out/review-0002.diff").to_path_buf(),
            ]
        );
    }

    #[test]
    fn output_validation_rejects_empty_and_directory_paths() {
        assert_eq!(
            validate_patch_output_path(Path::new("")),
            Err(PatchPathError::Empty)
        );
        let temp = std::env::temp_dir();
        assert_eq!(
            validate_patch_output_path(&temp),
            Err(PatchPathError::Directory)
        );
    }

    #[test]
    fn modifier_mapping_matches_windows_multi_selection() {
        assert_eq!(
            selection_gesture(false, false),
            PatchSelectionGesture::Plain
        );
        assert_eq!(
            selection_gesture(true, false),
            PatchSelectionGesture::Toggle
        );
        assert_eq!(
            selection_gesture(false, true),
            PatchSelectionGesture::ReplaceRange
        );
        assert_eq!(
            selection_gesture(true, true),
            PatchSelectionGesture::AddRange
        );
    }
}
