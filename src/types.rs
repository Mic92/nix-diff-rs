use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StorePath {
    pub path_str: String,
}

impl StorePath {
    pub fn new(path: PathBuf) -> Self {
        StorePath {
            path_str: path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub outputs: BTreeMap<String, Output>,
    pub input_sources: BTreeSet<StorePath>,
    pub input_derivations: BTreeMap<StorePath, BTreeSet<String>>,
    pub platform: String,
    pub builder: StorePath,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub path: StorePath,
    pub hash_algorithm: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DerivationDiff {
    Changed {
        original: Derivation,
        new: Derivation,
        outputs: OutputsDiff,
        platform: Option<StringDiff>,
        builder: Option<StringDiff>,
        args: Option<ArgumentsDiff>,
        sources: Option<SourcesDiff>,
        inputs: Option<InputsDiff>,
        env: Option<EnvironmentDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputsDiff {
    Identical,
    Changed(Vec<OutputDiff>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputDiff {
    pub name: String,
    pub diff: OutputDetailDiff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputDetailDiff {
    Added(Output),
    Removed(Output),
    Changed {
        old: Output,
        new: Box<Output>,
        path: Option<StringDiff>,
        hash_algo: Option<StringDiff>,
        hash: Option<StringDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentsDiff {
    Identical,
    Changed(Vec<StringDiff>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourcesDiff {
    Identical,
    Changed {
        added: Vec<StorePath>,
        removed: Vec<StorePath>,
        common: Vec<SourceDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiff {
    pub path: StorePath,
    pub diff: TextDiff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputsDiff {
    Identical,
    Changed {
        added: Vec<StorePath>,
        removed: Vec<StorePath>,
        changed: Vec<InputDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDiff {
    pub path: StorePath,
    pub outputs: Option<OutputSetDiff>,
    pub derivation: Option<Box<DerivationDiff>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputSetDiff {
    Added(BTreeSet<String>),
    Removed(BTreeSet<String>),
    Changed {
        added: BTreeSet<String>,
        removed: BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentDiff {
    Identical,
    Changed(BTreeMap<String, Option<EnvVarDiff>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvVarDiff {
    Added(String),
    Removed(String),
    Changed(StringDiff),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringDiff {
    Identical,
    Changed { old: String, new: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextDiff {
    Binary,
    Text(Vec<DiffLine>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DiffOrientation {
    #[default]
    Line,
    Word,
    Character,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ColorMode {
    Always,
    #[default]
    Auto,
    Never,
}
