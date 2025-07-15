use crate::types::*;
use anyhow::Result;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

pub struct DiffContext {
    already_compared: HashSet<(String, String)>,
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
        path1: &Path,
        path2: &Path,
        drv1: &Derivation,
        drv2: &Derivation,
    ) -> Result<DerivationDiff> {
        let key = (
            path1.to_string_lossy().to_string(),
            path2.to_string_lossy().to_string(),
        );

        if self.already_compared.contains(&key) {
            return Ok(DerivationDiff::Changed {
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
        let platform = self.diff_strings(&drv1.platform, &drv2.platform);
        let builder = self.diff_strings(
            &drv1.builder.path.to_string_lossy(),
            &drv2.builder.path.to_string_lossy(),
        );
        let args = self.diff_arguments(&drv1.args, &drv2.args);
        let sources = self.diff_sources(&drv1.input_sources, &drv2.input_sources)?;
        let inputs = self.diff_inputs(&drv1.input_derivations, &drv2.input_derivations)?;
        let env = self.diff_environment(&drv1.env, &drv2.env);

        Ok(DerivationDiff::Changed {
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
        outputs1: &BTreeMap<String, Output>,
        outputs2: &BTreeMap<String, Output>,
    ) -> OutputsDiff {
        let mut diffs = Vec::new();

        let all_names: BTreeSet<_> = outputs1.keys().chain(outputs2.keys()).cloned().collect();

        for name in all_names {
            match (outputs1.get(&name), outputs2.get(&name)) {
                (Some(o1), Some(o2)) if o1 != o2 => {
                    let path_diff = self.diff_strings(
                        &o1.path.path.to_string_lossy(),
                        &o2.path.path.to_string_lossy(),
                    );
                    let hash_algo_diff =
                        self.diff_optional_strings(&o1.hash_algorithm, &o2.hash_algorithm);
                    let hash_diff = self.diff_optional_strings(&o1.hash, &o2.hash);

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

    fn diff_arguments(&self, args1: &[String], args2: &[String]) -> Option<ArgumentsDiff> {
        if args1 == args2 {
            return None;
        }

        let mut diffs = Vec::new();
        let max_len = args1.len().max(args2.len());

        for i in 0..max_len {
            let arg1 = args1.get(i).map(|s| s.as_str()).unwrap_or("");
            let arg2 = args2.get(i).map(|s| s.as_str()).unwrap_or("");

            if let Some(diff) = self.diff_strings(arg1, arg2) {
                diffs.push(diff);
            }
        }

        Some(ArgumentsDiff::Changed(diffs))
    }

    fn diff_sources(
        &self,
        sources1: &BTreeSet<StorePath>,
        sources2: &BTreeSet<StorePath>,
    ) -> Result<Option<SourcesDiff>> {
        let added: Vec<_> = sources2.difference(sources1).cloned().collect();
        let removed: Vec<_> = sources1.difference(sources2).cloned().collect();
        let common_paths: Vec<_> = sources1.intersection(sources2).cloned().collect();

        let mut common = Vec::new();
        for path in common_paths {
            if let Ok(content1) = fs::read(&path.path) {
                if let Ok(content2) = fs::read(&path.path) {
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

        if added.is_empty() && removed.is_empty() && common.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SourcesDiff::Changed {
                added,
                removed,
                common,
            }))
        }
    }

    fn diff_inputs(
        &mut self,
        inputs1: &BTreeMap<StorePath, BTreeSet<String>>,
        inputs2: &BTreeMap<StorePath, BTreeSet<String>>,
    ) -> Result<Option<InputsDiff>> {
        let keys1: BTreeSet<_> = inputs1.keys().cloned().collect();
        let keys2: BTreeSet<_> = inputs2.keys().cloned().collect();

        let added: Vec<_> = keys2.difference(&keys1).cloned().collect();
        let removed: Vec<_> = keys1.difference(&keys2).cloned().collect();

        let mut changed = Vec::new();
        for path in keys1.intersection(&keys2) {
            let outputs1 = &inputs1[path];
            let outputs2 = &inputs2[path];

            let outputs_diff = if outputs1 != outputs2 {
                let added_outputs: BTreeSet<_> = outputs2.difference(outputs1).cloned().collect();
                let removed_outputs: BTreeSet<_> = outputs1.difference(outputs2).cloned().collect();

                if !added_outputs.is_empty() || !removed_outputs.is_empty() {
                    Some(OutputSetDiff::Changed {
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
            let derivation_diff = if let (Ok(drv1), Ok(drv2)) = (
                crate::parser::parse_derivation(&path.path),
                crate::parser::parse_derivation(&path.path),
            ) {
                if drv1 != drv2 {
                    Some(Box::new(
                        self.diff_derivations(&path.path, &path.path, &drv1, &drv2)?,
                    ))
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
            Ok(Some(InputsDiff::Changed {
                added,
                removed,
                changed,
            }))
        }
    }

    fn diff_environment(
        &self,
        env1: &BTreeMap<String, String>,
        env2: &BTreeMap<String, String>,
    ) -> Option<EnvironmentDiff> {
        let mut diffs = BTreeMap::new();

        let all_keys: BTreeSet<_> = env1.keys().chain(env2.keys()).cloned().collect();

        for key in all_keys {
            match (env1.get(&key), env2.get(&key)) {
                (Some(v1), Some(v2)) if v1 != v2 => {
                    if let Some(diff) = self.diff_strings(v1, v2) {
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

        if diffs.is_empty() {
            None
        } else {
            Some(EnvironmentDiff::Changed(diffs))
        }
    }

    fn diff_strings(&self, s1: &str, s2: &str) -> Option<StringDiff> {
        if s1 == s2 {
            None
        } else {
            Some(StringDiff::Changed {
                old: s1.to_string(),
                new: s2.to_string(),
            })
        }
    }

    fn diff_optional_strings(
        &self,
        s1: &Option<String>,
        s2: &Option<String>,
    ) -> Option<StringDiff> {
        match (s1, s2) {
            (Some(a), Some(b)) => self.diff_strings(a, b),
            (None, None) => None,
            (Some(a), None) => Some(StringDiff::Changed {
                old: a.clone(),
                new: String::new(),
            }),
            (None, Some(b)) => Some(StringDiff::Changed {
                old: String::new(),
                new: b.clone(),
            }),
        }
    }

    fn diff_file_contents(&self, content1: &[u8], content2: &[u8]) -> TextDiff {
        // Check if content is binary
        if content1.contains(&0) || content2.contains(&0) {
            return TextDiff::Binary;
        }

        let text1 = String::from_utf8_lossy(content1);
        let text2 = String::from_utf8_lossy(content2);

        let diff = match self.orientation {
            DiffOrientation::Line => SimilarTextDiff::from_lines(&text1, &text2),
            DiffOrientation::Word => SimilarTextDiff::from_words(&text1, &text2),
            DiffOrientation::Character => SimilarTextDiff::from_chars(&text1, &text2),
        };

        let mut lines = Vec::new();
        for change in diff.iter_all_changes() {
            let line = change.value().to_string();
            match change.tag() {
                ChangeTag::Equal => lines.push(DiffLine::Context(line)),
                ChangeTag::Insert => lines.push(DiffLine::Added(line)),
                ChangeTag::Delete => lines.push(DiffLine::Removed(line)),
            }
        }

        TextDiff::Text(lines)
    }
}
