use crate::types::*;
use anyhow::Result;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;

pub struct DiffContext {
    already_compared: HashSet<(Vec<u8>, Vec<u8>)>,
    orientation: DiffOrientation,
    #[allow(dead_code)]
    context_lines: usize,
}

impl DiffContext {
    pub fn new(orientation: DiffOrientation, context_lines: usize) -> Self {
        DiffContext {
            already_compared: HashSet::new(),
            orientation,
            context_lines,
        }
    }

    pub fn diff_derivations(
        &mut self,
        path1: &[u8],
        path2: &[u8],
        drv1: &Derivation,
        drv2: &Derivation,
    ) -> Result<DerivationDiff> {
        let key = (path1.to_vec(), path2.to_vec());

        if self.already_compared.contains(&key) {
            return Ok(DerivationDiff {
                original: drv1.clone(),
                new: drv2.clone(),
                outputs: OutputsDiff::Identical,
                platform: None,
                builder: None,
                args: None,
                sources: None,
                inputs: None,
                env: None,
            });
        }

        self.already_compared.insert(key);

        let outputs = self.diff_outputs(&drv1.outputs, &drv2.outputs);
        let platform = self.diff_bytes(&drv1.platform, &drv2.platform);
        let builder = self.diff_bytes(&drv1.builder, &drv2.builder);
        let args = self.diff_arguments(&drv1.args, &drv2.args);
        let sources = self.diff_sources(&drv1.input_sources, &drv2.input_sources)?;
        let inputs = self.diff_inputs(&drv1.input_derivations, &drv2.input_derivations)?;
        let env = self.diff_environment(&drv1.env, &drv2.env);

        Ok(DerivationDiff {
            original: drv1.clone(),
            new: drv2.clone(),
            outputs,
            platform,
            builder,
            args,
            sources,
            inputs,
            env,
        })
    }

    fn diff_outputs(
        &self,
        outputs1: &BTreeMap<Vec<u8>, Output>,
        outputs2: &BTreeMap<Vec<u8>, Output>,
    ) -> OutputsDiff {
        let mut diffs = Vec::new();

        let all_names: BTreeSet<_> = outputs1.keys().chain(outputs2.keys()).cloned().collect();

        for name in all_names {
            match (outputs1.get(&name), outputs2.get(&name)) {
                (Some(o1), Some(o2)) if o1 != o2 => {
                    let path_diff = self.diff_bytes(&o1.path, &o2.path);
                    let hash_algo_diff =
                        self.diff_optional_bytes(&o1.hash_algorithm, &o2.hash_algorithm);
                    let hash_diff = self.diff_optional_bytes(&o1.hash, &o2.hash);

                    diffs.push(OutputDiff {
                        name: name.clone(),
                        diff: OutputDetailDiff::Changed {
                            old: o1.clone(),
                            new: Box::new(o2.clone()),
                            path: path_diff,
                            hash_algo: hash_algo_diff,
                            hash: hash_diff,
                        },
                    });
                }
                (Some(o), None) => {
                    diffs.push(OutputDiff {
                        name: name.clone(),
                        diff: OutputDetailDiff::Removed(o.clone()),
                    });
                }
                (None, Some(o)) => {
                    diffs.push(OutputDiff {
                        name: name.clone(),
                        diff: OutputDetailDiff::Added(o.clone()),
                    });
                }
                _ => {}
            }
        }

        if diffs.is_empty() {
            OutputsDiff::Identical
        } else {
            OutputsDiff::Changed(diffs)
        }
    }

    fn diff_arguments(&self, args1: &[Vec<u8>], args2: &[Vec<u8>]) -> Option<ArgumentsDiff> {
        if args1 == args2 {
            return None;
        }

        let mut diffs = Vec::new();
        let max_len = args1.len().max(args2.len());

        for i in 0..max_len {
            let arg1 = args1.get(i).map(|s| s.as_slice()).unwrap_or(b"");
            let arg2 = args2.get(i).map(|s| s.as_slice()).unwrap_or(b"");

            if arg1 != arg2 {
                diffs.push(ArgumentDiff {
                    index: i,
                    diff: StringDiff {
                        old: arg1.to_vec(),
                        new: arg2.to_vec(),
                    },
                });
            }
        }

        if diffs.is_empty() { None } else { Some(diffs) }
    }

    fn diff_sources(
        &self,
        sources1: &BTreeSet<Vec<u8>>,
        sources2: &BTreeSet<Vec<u8>>,
    ) -> Result<Option<SourcesDiff>> {
        // Extract name from a store path: /nix/store/hash-name -> name
        fn get_source_name(path: &[u8]) -> &[u8] {
            if let Some(last_slash) = path.iter().rposition(|&b| b == b'/') {
                let filename = &path[last_slash + 1..];
                if let Some(dash_pos) = filename.iter().position(|&b| b == b'-') {
                    return &filename[dash_pos + 1..];
                }
            }
            path
        }

        // Group paths by name so we can pair sources that changed hash
        let mut by_name1: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new();
        let mut by_name2: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new();
        for p in sources1 {
            by_name1
                .entry(get_source_name(p).to_vec())
                .or_default()
                .insert(p.clone());
        }
        for p in sources2 {
            by_name2
                .entry(get_source_name(p).to_vec())
                .or_default()
                .insert(p.clone());
        }

        let all_names: BTreeSet<_> = by_name1.keys().chain(by_name2.keys()).cloned().collect();

        let mut added = BTreeSet::new();
        let mut removed = BTreeSet::new();
        let mut common = Vec::new();

        let empty = BTreeSet::new();
        for name in &all_names {
            let paths1 = by_name1.get(name).unwrap_or(&empty);
            let paths2 = by_name2.get(name).unwrap_or(&empty);

            let only1: Vec<_> = paths1.difference(paths2).cloned().collect();
            let only2: Vec<_> = paths2.difference(paths1).cloned().collect();

            let pair_count = only1.len().min(only2.len());
            for i in 0..pair_count {
                let p1 = &only1[i];
                let p2 = &only2[i];
                match (
                    std::str::from_utf8(p1).ok().and_then(|s| fs::read(s).ok()),
                    std::str::from_utf8(p2).ok().and_then(|s| fs::read(s).ok()),
                ) {
                    (Some(c1), Some(c2)) => {
                        if c1 != c2 {
                            common.push(SourceDiff {
                                path: name.clone(),
                                diff: self.diff_file_contents(&c1, &c2),
                            });
                        }
                    }
                    _ => {
                        // Cannot read — fall back to reporting as added/removed
                        removed.insert(p1.clone());
                        added.insert(p2.clone());
                    }
                }
            }
            for p in &only1[pair_count..] {
                removed.insert(p.clone());
            }
            for p in &only2[pair_count..] {
                added.insert(p.clone());
            }
        }

        if added.is_empty() && removed.is_empty() && common.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SourcesDiff {
                added,
                removed,
                common,
            }))
        }
    }

    fn diff_inputs(
        &mut self,
        inputs1: &BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>,
        inputs2: &BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>,
    ) -> Result<Option<InputsDiff>> {
        // Extract derivation name from a path like /nix/store/hash-name.drv -> name.drv
        fn get_derivation_name(path: &[u8]) -> &[u8] {
            if let Some(last_slash) = path.iter().rposition(|&b| b == b'/') {
                let filename = &path[last_slash + 1..];
                if let Some(dash_pos) = filename.iter().position(|&b| b == b'-') {
                    return &filename[dash_pos + 1..];
                }
            }
            path
        }

        // Build maps from derivation name to paths for both sets. A derivation
        // can have multiple inputs with the same name but different hashes,
        // so we collect all paths per name instead of overwriting.
        let mut names_to_paths1: HashMap<Vec<u8>, BTreeSet<Vec<u8>>> = HashMap::new();
        let mut names_to_paths2: HashMap<Vec<u8>, BTreeSet<Vec<u8>>> = HashMap::new();

        for path in inputs1.keys() {
            let name = get_derivation_name(path).to_vec();
            names_to_paths1
                .entry(name)
                .or_default()
                .insert(path.clone());
        }

        for path in inputs2.keys() {
            let name = get_derivation_name(path).to_vec();
            names_to_paths2
                .entry(name)
                .or_default()
                .insert(path.clone());
        }

        let all_names: BTreeSet<Vec<u8>> = names_to_paths1
            .keys()
            .chain(names_to_paths2.keys())
            .cloned()
            .collect();

        let mut added = BTreeSet::new();
        let mut removed = BTreeSet::new();
        let mut changed = Vec::new();

        let empty: BTreeSet<Vec<u8>> = BTreeSet::new();
        for name in all_names {
            let paths1 = names_to_paths1.get(&name).unwrap_or(&empty);
            let paths2 = names_to_paths2.get(&name).unwrap_or(&empty);

            // Paths present in both are unchanged at the path level (may still
            // have output-set differences, handled below). Paths only on one
            // side are candidates for matching.
            let only1: Vec<_> = paths1.difference(paths2).cloned().collect();
            let only2: Vec<_> = paths2.difference(paths1).cloned().collect();
            let common: Vec<_> = paths1.intersection(paths2).cloned().collect();

            // Pair up singletons on each side as "changed". If counts differ,
            // the extras are added/removed. We pair in sorted order which is
            // deterministic; without content inspection we cannot do better.
            let pair_count = only1.len().min(only2.len());
            for i in 0..pair_count {
                let path1 = &only1[i];
                let path2 = &only2[i];
                self.push_changed_input(
                    &name,
                    path1,
                    path2,
                    &inputs1[path1],
                    &inputs2[path2],
                    &mut changed,
                )?;
            }
            for path1 in &only1[pair_count..] {
                removed.insert(DerivationPath(path1.clone()));
            }
            for path2 in &only2[pair_count..] {
                added.insert(DerivationPath(path2.clone()));
            }

            // Same-path inputs: check for output-set changes
            for path in &common {
                let outputs1 = &inputs1[path];
                let outputs2 = &inputs2[path];
                if outputs1 != outputs2 {
                    let added_outputs: BTreeSet<_> =
                        outputs2.difference(outputs1).cloned().collect();
                    let removed_outputs: BTreeSet<_> =
                        outputs1.difference(outputs2).cloned().collect();
                    if !added_outputs.is_empty() || !removed_outputs.is_empty() {
                        changed.push(InputDiff {
                            path: name.clone(),
                            outputs: Some(OutputSetDiff {
                                added: added_outputs,
                                removed: removed_outputs,
                            }),
                            derivation: None,
                        });
                    }
                }
            }
        }

        if added.is_empty() && removed.is_empty() && changed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(InputsDiff {
                added,
                removed,
                changed,
            }))
        }
    }

    fn push_changed_input(
        &mut self,
        name: &[u8],
        path1: &[u8],
        path2: &[u8],
        outputs1: &BTreeSet<Vec<u8>>,
        outputs2: &BTreeSet<Vec<u8>>,
        changed: &mut Vec<InputDiff>,
    ) -> Result<()> {
        let outputs_diff = if outputs1 != outputs2 {
            let added_outputs: BTreeSet<_> = outputs2.difference(outputs1).cloned().collect();
            let removed_outputs: BTreeSet<_> = outputs1.difference(outputs2).cloned().collect();
            if !added_outputs.is_empty() || !removed_outputs.is_empty() {
                Some(OutputSetDiff {
                    added: added_outputs,
                    removed: removed_outputs,
                })
            } else {
                None
            }
        } else {
            None
        };

        // Try to load and recursively diff the derivations
        let derivation_diff =
            if let (Ok(p1), Ok(p2)) = (std::str::from_utf8(path1), std::str::from_utf8(path2)) {
                if let (Ok(drv1), Ok(drv2)) = (
                    crate::parser::parse_derivation(p1),
                    crate::parser::parse_derivation(p2),
                ) {
                    Some(Box::new(self.diff_derivations(path1, path2, &drv1, &drv2)?))
                } else {
                    None
                }
            } else {
                None
            };

        changed.push(InputDiff {
            path: name.to_vec(),
            outputs: outputs_diff,
            derivation: derivation_diff,
        });
        Ok(())
    }

    fn diff_environment(
        &self,
        env1: &BTreeMap<Vec<u8>, Vec<u8>>,
        env2: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> Option<EnvironmentDiff> {
        let mut diffs = BTreeMap::new();

        let all_keys: BTreeSet<_> = env1.keys().chain(env2.keys()).cloned().collect();

        for key in all_keys {
            match (env1.get(&key), env2.get(&key)) {
                (Some(v1), Some(v2)) if v1 != v2 => {
                    if let Some(diff) = self.diff_bytes(v1, v2) {
                        diffs.insert(key, Some(EnvVarDiff::Changed(diff)));
                    }
                }
                (Some(v), None) => {
                    diffs.insert(key, Some(EnvVarDiff::Removed(v.clone())));
                }
                (None, Some(v)) => {
                    diffs.insert(key, Some(EnvVarDiff::Added(v.clone())));
                }
                _ => {}
            }
        }

        if diffs.is_empty() { None } else { Some(diffs) }
    }

    fn diff_bytes(&self, s1: &[u8], s2: &[u8]) -> Option<StringDiff> {
        if s1 == s2 {
            None
        } else {
            Some(StringDiff {
                old: s1.to_vec(),
                new: s2.to_vec(),
            })
        }
    }

    fn diff_optional_bytes(
        &self,
        s1: &Option<Vec<u8>>,
        s2: &Option<Vec<u8>>,
    ) -> Option<StringDiff> {
        match (s1, s2) {
            (Some(a), Some(b)) => self.diff_bytes(a, b),
            (None, None) => None,
            (Some(a), None) => Some(StringDiff {
                old: a.clone(),
                new: Vec::new(),
            }),
            (None, Some(b)) => Some(StringDiff {
                old: Vec::new(),
                new: b.clone(),
            }),
        }
    }

    fn diff_file_contents(&self, content1: &[u8], content2: &[u8]) -> TextDiff {
        // Check if content is binary
        if content1.contains(&0) || content2.contains(&0) {
            return TextDiff::Binary;
        }

        let diff = match self.orientation {
            DiffOrientation::Line => SimilarTextDiff::from_lines(content1, content2),
            DiffOrientation::Word => SimilarTextDiff::from_words(content1, content2),
            DiffOrientation::Character => SimilarTextDiff::from_chars(content1, content2),
        };

        let mut lines = Vec::new();
        for change in diff.iter_all_changes() {
            let line = change.value().to_vec();
            match change.tag() {
                ChangeTag::Equal => lines.push(DiffLine::Context(line)),
                ChangeTag::Insert => lines.push(DiffLine::Added(line)),
                ChangeTag::Delete => lines.push(DiffLine::Removed(line)),
            }
        }

        TextDiff::Text(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> DiffContext {
        DiffContext::new(DiffOrientation::Line, 3)
    }

    #[test]
    fn diff_sources_matches_by_name_and_diffs_contents() {
        // Sources with the same name but different store hashes should be
        // paired and their file contents compared. Previously the code
        // iterated the intersection of full paths (always empty when hashes
        // differ) and read the same file twice.
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("store");
        std::fs::create_dir_all(&store).unwrap();

        let p1 = store.join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-script.sh");
        let p2 = store.join("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-script.sh");
        std::fs::write(&p1, b"echo old\n").unwrap();
        std::fs::write(&p2, b"echo new\n").unwrap();

        let s1: BTreeSet<Vec<u8>> = [p1.to_string_lossy().as_bytes().to_vec()].into();
        let s2: BTreeSet<Vec<u8>> = [p2.to_string_lossy().as_bytes().to_vec()].into();

        let diff = ctx().diff_sources(&s1, &s2).unwrap().unwrap();

        assert!(diff.added.is_empty(), "expected name-match, not addition");
        assert!(diff.removed.is_empty(), "expected name-match, not removal");
        assert_eq!(diff.common.len(), 1, "expected one content diff");
        match &diff.common[0].diff {
            TextDiff::Text(lines) => {
                assert!(
                    lines
                        .iter()
                        .any(|l| matches!(l, DiffLine::Removed(t) if t.starts_with(b"echo old")))
                );
                assert!(
                    lines
                        .iter()
                        .any(|l| matches!(l, DiffLine::Added(t) if t.starts_with(b"echo new")))
                );
            }
            _ => panic!("expected text diff"),
        }
    }

    #[test]
    fn diff_inputs_handles_duplicate_names() {
        // Two input derivations can share the same name with different hashes
        // (e.g., two "source.drv" inputs). The name-based matching must not
        // silently drop one of them.
        let mut inputs1: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new();
        inputs1.insert(
            b"/nix/store/aaaa-source.drv".to_vec(),
            [b"out".to_vec()].into(),
        );
        inputs1.insert(
            b"/nix/store/bbbb-source.drv".to_vec(),
            [b"out".to_vec()].into(),
        );

        // Second derivation has the same two inputs, unchanged
        let inputs2 = inputs1.clone();

        let diff = ctx().diff_inputs(&inputs1, &inputs2).unwrap();
        // Identical inputs → no diff. With the bug, one input is dropped from
        // each map and the survivor is compared against itself, still yielding
        // None — so also assert we account for both paths when they differ:
        assert!(diff.is_none());

        // Now remove one from inputs2 — the diff must report exactly one removal
        let mut inputs2 = inputs1.clone();
        inputs2.remove(b"/nix/store/bbbb-source.drv".as_slice());

        let diff = ctx().diff_inputs(&inputs1, &inputs2).unwrap().unwrap();
        assert_eq!(diff.removed.len(), 1, "expected exactly one removed input");
        assert!(diff.added.is_empty());
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn diff_arguments_preserves_positional_index() {
        // Only argument at index 1 differs. The diff must record index 1,
        // not compact to index 0, so the renderer can show the correct position.
        let args1 = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        let args2 = vec![b"a".to_vec(), b"X".to_vec(), b"c".to_vec()];

        let diffs = ctx().diff_arguments(&args1, &args2).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].index, 1);
        assert_eq!(diffs[0].diff.old, b"b");
        assert_eq!(diffs[0].diff.new, b"X");
    }
}
