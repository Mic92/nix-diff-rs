use crate::types::*;
use anyhow::Result;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::collections::{BTreeMap, BTreeSet, HashSet};
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
                diffs.push(StringDiff {
                    old: arg1.to_vec(),
                    new: arg2.to_vec(),
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
        let added: BTreeSet<_> = sources2.difference(sources1).cloned().collect();
        let removed: BTreeSet<_> = sources1.difference(sources2).cloned().collect();
        let common_paths: Vec<_> = sources1.intersection(sources2).cloned().collect();

        let mut common = Vec::new();
        for path in common_paths {
            // Convert bytes to path for file system operations
            if let Ok(path_str) = std::str::from_utf8(&path) {
                if let Ok(content1) = fs::read(path_str) {
                    if let Ok(content2) = fs::read(path_str) {
                        if content1 != content2 {
                            let diff = self.diff_file_contents(&content1, &content2);
                            common.push(SourceDiff {
                                path: path.clone(),
                                diff,
                            });
                        }
                    }
                }
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
        let keys1: BTreeSet<_> = inputs1.keys().cloned().collect();
        let keys2: BTreeSet<_> = inputs2.keys().cloned().collect();

        let added: BTreeSet<DerivationPath> = keys2
            .difference(&keys1)
            .map(|k| DerivationPath(k.clone()))
            .collect();
        let removed: BTreeSet<DerivationPath> = keys1
            .difference(&keys2)
            .map(|k| DerivationPath(k.clone()))
            .collect();

        let mut changed = Vec::new();
        for path in keys1.intersection(&keys2) {
            let outputs1 = &inputs1[path];
            let outputs2 = &inputs2[path];

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

            // Try to load and compare the derivations
            let derivation_diff = if let Ok(path_str) = std::str::from_utf8(path) {
                if let (Ok(drv1), Ok(drv2)) = (
                    crate::parser::parse_derivation(path_str),
                    crate::parser::parse_derivation(path_str),
                ) {
                    if drv1 != drv2 {
                        Some(Box::new(self.diff_derivations(path, path, &drv1, &drv2)?))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if outputs_diff.is_some() || derivation_diff.is_some() {
                changed.push(InputDiff {
                    path: path.clone(),
                    outputs: outputs_diff,
                    derivation: derivation_diff,
                });
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
