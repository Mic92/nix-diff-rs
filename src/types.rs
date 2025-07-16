use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub outputs: BTreeMap<Vec<u8>, Output>,
    pub input_sources: BTreeSet<Vec<u8>>,
    pub input_derivations: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>,
    pub platform: Vec<u8>,
    pub builder: Vec<u8>,
    pub args: Vec<Vec<u8>>,
    pub env: BTreeMap<Vec<u8>, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub path: Vec<u8>,
    pub hash_algorithm: Option<Vec<u8>>,
    pub hash: Option<Vec<u8>>,
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
    pub name: Vec<u8>,
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
        added: Vec<Vec<u8>>,
        removed: Vec<Vec<u8>>,
        common: Vec<SourceDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiff {
    pub path: Vec<u8>,
    pub diff: TextDiff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputsDiff {
    Identical,
    Changed {
        added: Vec<Vec<u8>>,
        removed: Vec<Vec<u8>>,
        changed: Vec<InputDiff>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDiff {
    pub path: Vec<u8>,
    pub outputs: Option<OutputSetDiff>,
    pub derivation: Option<Box<DerivationDiff>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputSetDiff {
    Added(BTreeSet<Vec<u8>>),
    Removed(BTreeSet<Vec<u8>>),
    Changed {
        added: BTreeSet<Vec<u8>>,
        removed: BTreeSet<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentDiff {
    Identical,
    Changed(BTreeMap<Vec<u8>, Option<EnvVarDiff>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvVarDiff {
    Added(Vec<u8>),
    Removed(Vec<u8>),
    Changed(StringDiff),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringDiff {
    Identical,
    Changed { old: Vec<u8>, new: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextDiff {
    Binary,
    Text(Vec<DiffLine>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    Context(Vec<u8>),
    Added(Vec<u8>),
    Removed(Vec<u8>),
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
