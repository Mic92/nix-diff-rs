use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub outputs: BTreeMap<String, Output>,
    pub input_sources: BTreeSet<String>,
    pub input_derivations: BTreeMap<String, BTreeSet<String>>,
    pub platform: String,
    pub builder: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub path: String,
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
        added: Vec<String>,
        removed: Vec<String>,
        common: Vec<SourceDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiff {
    pub path: String,
    pub diff: TextDiff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputsDiff {
    Identical,
    Changed {
        added: Vec<String>,
        removed: Vec<String>,
        changed: Vec<InputDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDiff {
    pub path: String,
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
