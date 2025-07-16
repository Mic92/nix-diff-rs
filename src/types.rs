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
pub struct DerivationDiff {
    pub original: Derivation,
    pub new: Derivation,
    pub outputs: OutputsDiff,
    pub platform: Option<StringDiff>,
    pub builder: Option<StringDiff>,
    pub args: Option<ArgumentsDiff>,
    pub sources: Option<SourcesDiff>,
    pub inputs: Option<InputsDiff>,
    pub env: Option<EnvironmentDiff>,
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

pub type ArgumentsDiff = Vec<StringDiff>;

#[derive(Debug, Clone, PartialEq)]
pub struct SourcesDiff {
    pub added: Vec<Vec<u8>>,
    pub removed: Vec<Vec<u8>>,
    pub common: Vec<SourceDiff>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiff {
    pub path: Vec<u8>,
    pub diff: TextDiff,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputsDiff {
    pub added: Vec<Vec<u8>>,
    pub removed: Vec<Vec<u8>>,
    pub changed: Vec<InputDiff>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDiff {
    pub path: Vec<u8>,
    pub outputs: Option<OutputSetDiff>,
    pub derivation: Option<Box<DerivationDiff>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSetDiff {
    pub added: BTreeSet<Vec<u8>>,
    pub removed: BTreeSet<Vec<u8>>,
}

pub type EnvironmentDiff = BTreeMap<Vec<u8>, Option<EnvVarDiff>>;

#[derive(Debug, Clone, PartialEq)]
pub enum EnvVarDiff {
    Added(Vec<u8>),
    Removed(Vec<u8>),
    Changed(StringDiff),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringDiff {
    pub old: Vec<u8>,
    pub new: Vec<u8>,
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
