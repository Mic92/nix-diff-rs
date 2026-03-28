use crate::types::*;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::io::{self, IsTerminal, Write};

const RED: &[u8] = b"\x1b[31m";
const GREEN: &[u8] = b"\x1b[32m";
const YELLOW: &[u8] = b"\x1b[33m";
const CYAN: &[u8] = b"\x1b[36m";
const BOLD: &[u8] = b"\x1b[1m";
const DIM: &[u8] = b"\x1b[2m";
const REVERSE: &[u8] = b"\x1b[7m";
const NOREVERSE: &[u8] = b"\x1b[27m";
const RESET: &[u8] = b"\x1b[0m";

macro_rules! extend {
    ($output:expr, $($data:expr),+ $(,)?) => {
        $(
            $output.extend_from_slice($data);
        )+
    };
}

pub struct Renderer {
    use_color: bool,
    context_lines: usize,
    verbose: bool,
    input_list_limit: usize,
    max_depth: Option<usize>,
    inline_highlight: bool,
}

impl Renderer {
    pub fn new(opts: RenderOptions) -> Self {
        // Per https://no-color.org/, only a non-empty NO_COLOR disables color.
        let no_color = std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty());
        let use_color = !no_color
            && match opts.color_mode {
                ColorMode::Always => true,
                ColorMode::Never => false,
                ColorMode::Auto => io::stdout().is_terminal(),
            };
        Renderer {
            use_color,
            context_lines: opts.context_lines,
            verbose: opts.verbose,
            input_list_limit: opts.input_list_limit,
            max_depth: opts.max_depth,
            // Inline highlighting relies on reverse-video ANSI escapes;
            // without color it would just print the same text twice.
            inline_highlight: opts.inline_highlight && use_color,
        }
    }

    /// Render the diff to stdout.
    /// Returns `true` if the derivations differ, `false` if identical.
    pub fn render(&self, diff: &DerivationDiff, path1: &[u8], path2: &[u8]) -> io::Result<bool> {
        let mut stdout = io::stdout();
        let mut header = Vec::new();
        extend!(header, self.red(), b"--- ", path1, self.reset(), b"\n");
        extend!(header, self.green(), b"+++ ", path2, self.reset(), b"\n");
        let output = self.format_derivation_diff(diff, 0, 0);
        let differs = !output.is_empty();
        if differs {
            stdout.write_all(&header)?;
            stdout.write_all(&output)?;
        } else {
            stdout.write_all(b"The derivations are identical.\n")?;
        }
        stdout.flush()?;
        Ok(differs)
    }

    fn format_derivation_diff(
        &self,
        diff: &DerivationDiff,
        indent: usize,
        depth: usize,
    ) -> Vec<u8> {
        let mut output = Vec::new();

        let DerivationDiff {
            outputs,
            platform,
            builder,
            args,
            sources,
            inputs,
            env,
            ..
        } = diff;

        match outputs {
            OutputsDiff::Changed(output_diffs) => {
                // By default, hide output-path-only changes: if two derivations
                // differ at all, their output paths differ by construction.
                // Showing them just adds noise. We still show additions,
                // removals, and hash/algorithm changes (FOD hash updates).
                let interesting: Vec<_> = if self.verbose {
                    output_diffs.iter().collect()
                } else {
                    output_diffs
                        .iter()
                        .filter(|d| !is_path_only_change(&d.diff))
                        .collect()
                };
                if !interesting.is_empty() {
                    self.write_section(&mut output, b"Outputs", indent);
                    for out_diff in interesting {
                        self.format_output_diff(&mut output, out_diff, indent + 2);
                    }
                }
            }
            // AlreadyCompared is handled in format_inputs_diff so it can
            // be collapsed onto the same line as the • header.
            OutputsDiff::AlreadyCompared => return output,
            OutputsDiff::Identical => {}
        }

        if let Some(plat_diff) = platform {
            self.write_section(&mut output, b"Platform", indent);
            self.format_string_diff(&mut output, plat_diff, indent + 2);
        }

        if let Some(builder_diff) = builder {
            self.write_section(&mut output, b"Builder", indent);
            self.format_string_diff(&mut output, builder_diff, indent + 2);
        }

        if let Some(arg_diffs) = args {
            self.write_section(&mut output, b"Arguments", indent);
            for arg_diff in arg_diffs {
                self.write_indent(&mut output, indent + 2);
                extend!(
                    output,
                    b"Argument ",
                    arg_diff.index.to_string().as_bytes(),
                    b":\n"
                );
                // For multi-line arguments (like scripts), show them as a text diff
                let StringDiff { old, new } = &arg_diff.diff;
                if old.contains(&b'\n') || new.contains(&b'\n') {
                    self.format_text_diff(&mut output, old, new, indent + 4);
                } else {
                    self.format_string_diff(&mut output, &arg_diff.diff, indent + 4);
                }
            }
        }

        if let Some(src_diff) = sources {
            self.format_sources_diff(&mut output, src_diff, indent);
        }

        if let Some(inp_diff) = inputs {
            self.format_inputs_diff(&mut output, inp_diff, indent, depth);
        }

        if let Some(env_diffs) = env {
            // Filter env vars that merely mirror output paths (e.g. $out,
            // $dev) — they duplicate the Outputs section.
            let output_names: std::collections::HashSet<_> = diff
                .original
                .outputs
                .keys()
                .chain(diff.new.outputs.keys())
                .collect();
            let interesting: Vec<_> = env_diffs
                .iter()
                .filter_map(|(k, v)| v.as_ref().map(|d| (k, d)))
                .filter(|(k, _)| {
                    self.verbose
                        || (!output_names.contains(k)
                            // `builder` duplicates the Builder section.
                            && k.as_slice() != b"builder")
                })
                .collect();
            if !interesting.is_empty() {
                self.write_section(&mut output, b"Environment", indent);
                for (key, var_diff) in interesting {
                    self.write_indent(&mut output, indent + 2);
                    extend!(output, key, b":\n");
                    self.format_env_var_diff(&mut output, var_diff, indent + 4);
                }
            }
        }

        output
    }

    fn format_output_diff(&self, output: &mut Vec<u8>, diff: &OutputDiff, indent: usize) {
        self.write_indent(output, indent);
        extend!(output, b"Output '", &diff.name, b"':\n");

        match &diff.diff {
            OutputDetailDiff::Added(out) => {
                self.write_indent(output, indent + 2);
                extend!(
                    output,
                    self.green(),
                    b"+ Added: ",
                    &out.path,
                    self.reset(),
                    b"\n"
                );
            }
            OutputDetailDiff::Removed(out) => {
                self.write_indent(output, indent + 2);
                extend!(
                    output,
                    self.red(),
                    b"- Removed: ",
                    &out.path,
                    self.reset(),
                    b"\n"
                );
            }
            OutputDetailDiff::Changed {
                path,
                hash_algo,
                hash,
                ..
            } => {
                if let Some(path_diff) = path {
                    self.write_indent(output, indent + 2);
                    extend!(output, b"Path:\n");
                    self.format_string_diff(output, path_diff, indent + 4);
                }
                if let Some(algo_diff) = hash_algo {
                    self.write_indent(output, indent + 2);
                    extend!(output, b"Hash algorithm:\n");
                    self.format_string_diff(output, algo_diff, indent + 4);
                }
                if let Some(hash_diff) = hash {
                    self.write_indent(output, indent + 2);
                    extend!(output, b"Hash:\n");
                    self.format_string_diff(output, hash_diff, indent + 4);
                }
            }
        }
    }

    fn format_string_diff(&self, output: &mut Vec<u8>, diff: &StringDiff, indent: usize) {
        let StringDiff { old, new } = diff;
        if self.inline_highlight {
            // Single-line pair: run a word-level diff once and highlight only
            // the changed segments on each side. This makes store-path hash
            // changes and version bumps immediately visible.
            let old_toks = tokenize_path(old);
            let new_toks = tokenize_path(new);
            let ops = similar::capture_diff_slices(similar::Algorithm::Myers, &old_toks, &new_toks);
            self.write_inline_line(
                output,
                indent,
                self.red(),
                b"- ",
                &ops,
                &old_toks,
                &new_toks,
                true,
            );
            self.write_inline_line(
                output,
                indent,
                self.green(),
                b"+ ",
                &ops,
                &old_toks,
                &new_toks,
                false,
            );
        } else {
            self.write_indent(output, indent);
            extend!(output, self.red(), b"- ", old, self.reset(), b"\n");
            self.write_indent(output, indent);
            extend!(output, self.green(), b"+ ", new, self.reset(), b"\n");
        }
    }

    /// Write one side of an old/new pair with reverse-video highlighting on
    /// the segments that differ. Tokenization is done by the caller so the
    /// diff is computed once and reused for both sides.
    #[allow(clippy::too_many_arguments)]
    fn write_inline_line(
        &self,
        output: &mut Vec<u8>,
        indent: usize,
        color: &[u8],
        sign: &[u8],
        ops: &[similar::DiffOp],
        old_toks: &[&[u8]],
        new_toks: &[&[u8]],
        is_old: bool,
    ) {
        self.write_indent(output, indent);
        extend!(output, color, sign);
        // Track reverse-video state so adjacent emphasized tokens share a
        // single REVERSE/NOREVERSE pair instead of wrapping each token.
        let mut in_rev = false;
        for op in ops {
            for change in op.iter_changes(old_toks, new_toks) {
                let emit = match change.tag() {
                    ChangeTag::Equal => true,
                    ChangeTag::Delete => is_old,
                    ChangeTag::Insert => !is_old,
                };
                if !emit {
                    continue;
                }
                let emph = change.tag() != ChangeTag::Equal;
                if emph != in_rev {
                    output.extend_from_slice(if emph { REVERSE } else { NOREVERSE });
                    in_rev = emph;
                }
                output.extend_from_slice(change.value());
            }
        }
        if in_rev {
            output.extend_from_slice(NOREVERSE);
        }
        extend!(output, self.reset(), b"\n");
    }

    fn format_sources_diff(&self, output: &mut Vec<u8>, diff: &SourcesDiff, indent: usize) {
        let SourcesDiff {
            added,
            removed,
            common,
        } = diff;
        self.write_section(output, b"Sources", indent);

        for path in removed {
            self.write_indent(output, indent + 2);
            extend!(output, self.red(), b"- ", path, self.reset(), b"\n");
        }

        for path in added {
            self.write_indent(output, indent + 2);
            extend!(output, self.green(), b"+ ", path, self.reset(), b"\n");
        }

        for src_diff in common {
            self.write_indent(output, indent + 2);
            extend!(
                output,
                self.yellow(),
                b"~ ",
                &src_diff.path,
                self.reset(),
                b"\n"
            );
            match &src_diff.diff {
                TextDiff::Binary => {
                    self.write_indent(output, indent + 4);
                    extend!(
                        output,
                        self.yellow(),
                        b"Binary files differ",
                        self.reset(),
                        b"\n"
                    );
                }
                TextDiff::Text { old, new } => {
                    self.format_text_diff(output, old, new, indent + 4);
                }
            }
        }
    }

    fn format_inputs_diff(
        &self,
        output: &mut Vec<u8>,
        diff: &InputsDiff,
        indent: usize,
        depth: usize,
    ) {
        let InputsDiff {
            added,
            removed,
            changed,
        } = diff;

        // Only show section header if there are simple additions/removals
        if !added.is_empty() || !removed.is_empty() {
            self.write_section(output, b"Input derivations", indent);
            self.write_path_list(
                output,
                removed.iter().map(|p| &p.0),
                b"- ",
                self.red(),
                indent + 2,
            );
            self.write_path_list(
                output,
                added.iter().map(|p| &p.0),
                b"+ ",
                self.green(),
                indent + 2,
            );
        }

        // Show changed derivations with a compact • bullet header.
        for inp_diff in changed {
            let already = matches!(
                inp_diff.derivation.as_deref(),
                Some(DerivationDiff {
                    outputs: OutputsDiff::AlreadyCompared,
                    ..
                })
            );
            self.write_indent(output, indent);
            extend!(
                output,
                self.bold(),
                self.cyan(),
                b"\xe2\x80\xa2 ",
                &inp_diff.path,
                self.reset()
            );
            if already {
                extend!(output, self.dim(), b" (already compared)", self.reset());
            }
            output.push(b'\n');

            // Consumed-output changes are independent of the nested derivation
            // diff: they describe which outputs the *parent* consumes from this
            // input. Show them regardless of whether we also have a drv diff.
            if let Some(out_diff) = &inp_diff.outputs {
                self.write_indent(output, indent + 2);
                extend!(output, b"Consumed outputs:\n");
                self.format_output_set_diff(output, out_diff, indent + 4);
            }
            if let (Some(drv_diff), false) = (&inp_diff.derivation, already) {
                if self.max_depth.is_some_and(|d| depth + 1 > d) {
                    self.write_indent(output, indent + 2);
                    extend!(
                        output,
                        self.dim(),
                        b"(depth limit reached, use --depth to show more)",
                        self.reset(),
                        b"\n"
                    );
                } else {
                    let sub = self.format_derivation_diff(drv_diff, indent + 2, depth + 1);
                    extend!(output, &sub);
                }
            }
        }
    }

    fn format_output_set_diff(&self, output: &mut Vec<u8>, diff: &OutputSetDiff, indent: usize) {
        let OutputSetDiff { added, removed } = diff;
        for out in removed {
            self.write_indent(output, indent);
            extend!(output, self.red(), b"- ", out, self.reset(), b"\n");
        }
        for out in added {
            self.write_indent(output, indent);
            extend!(output, self.green(), b"+ ", out, self.reset(), b"\n");
        }
    }

    fn format_env_var_diff(&self, output: &mut Vec<u8>, diff: &EnvVarDiff, indent: usize) {
        match diff {
            EnvVarDiff::Added(value) => {
                self.write_indent(output, indent);
                extend!(output, self.green(), b"+ ", value, self.reset(), b"\n");
            }
            EnvVarDiff::Removed(value) => {
                self.write_indent(output, indent);
                extend!(output, self.red(), b"- ", value, self.reset(), b"\n");
            }
            EnvVarDiff::Changed(str_diff) => {
                let StringDiff { old, new } = str_diff;
                // For multi-line environment variables, show them as a text diff
                if old.contains(&b'\n') || new.contains(&b'\n') {
                    self.format_text_diff(output, old, new, indent);
                } else {
                    self.format_string_diff(output, str_diff, indent);
                }
            }
        }
    }

    /// Render a multi-line text diff with context trimming. When inline
    /// highlighting is enabled, changed words within changed lines are
    /// reverse-video'd (delta-style), making it obvious *what* in the line
    /// changed — particularly useful for store-path hash changes.
    fn format_text_diff(&self, output: &mut Vec<u8>, old: &[u8], new: &[u8], indent: usize) {
        let diff = SimilarTextDiff::from_lines(old, new);

        for (idx, group) in diff.grouped_ops(self.context_lines).iter().enumerate() {
            if idx > 0 {
                self.write_indent(output, indent);
                extend!(output, b"...\n");
            }
            for op in group {
                if self.inline_highlight {
                    for change in diff.iter_inline_changes(op) {
                        let (color, sign): (&[u8], &[u8]) = match change.tag() {
                            ChangeTag::Delete => (self.red(), b"- "),
                            ChangeTag::Insert => (self.green(), b"+ "),
                            ChangeTag::Equal => (b"", b"  "),
                        };
                        self.write_indent(output, indent);
                        extend!(output, color, sign);
                        for (emphasized, value) in change.iter_strings_lossy() {
                            let bytes = value.as_bytes();
                            // Strip trailing newline so reset comes before \n
                            // (avoids color bleed in some pagers).
                            let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
                            if emphasized {
                                extend!(output, REVERSE, body, NOREVERSE);
                            } else {
                                output.extend_from_slice(body);
                            }
                        }
                        extend!(output, self.reset(), b"\n");
                    }
                } else {
                    for change in diff.iter_changes(op) {
                        let (color, sign): (&[u8], &[u8]) = match change.tag() {
                            ChangeTag::Delete => (self.red(), b"- "),
                            ChangeTag::Insert => (self.green(), b"+ "),
                            ChangeTag::Equal => (b"", b"  "),
                        };
                        self.write_indent(output, indent);
                        let val = change.value();
                        let body = val.strip_suffix(b"\n").unwrap_or(val);
                        extend!(output, color, sign, body, self.reset(), b"\n");
                    }
                }
            }
        }
    }

    /// Write a list of store paths, truncating to `input_list_limit` entries
    /// and summarizing the remainder. Large add/remove lists (e.g., after a
    /// stdenv bump) otherwise dominate the output without adding insight.
    fn write_path_list<'a, I>(
        &self,
        output: &mut Vec<u8>,
        paths: I,
        sign: &[u8],
        color: &[u8],
        indent: usize,
    ) where
        I: Iterator<Item = &'a Vec<u8>>,
    {
        let mut shown = 0;
        let mut hidden = 0;
        for path in paths {
            if self.verbose || shown < self.input_list_limit {
                self.write_indent(output, indent);
                extend!(output, color, sign, path, self.reset(), b"\n");
                shown += 1;
            } else {
                hidden += 1;
            }
        }
        if hidden > 0 {
            self.write_indent(output, indent);
            extend!(
                output,
                self.dim(),
                sign,
                b"... and ",
                hidden.to_string().as_bytes(),
                b" more (use --verbose to show all)",
                self.reset(),
                b"\n"
            );
        }
    }

    fn write_section(&self, output: &mut Vec<u8>, title: &[u8], indent: usize) {
        self.write_indent(output, indent);
        extend!(output, self.bold(), title, b":", self.reset(), b"\n");
    }

    fn write_indent(&self, output: &mut Vec<u8>, indent: usize) {
        for _ in 0..indent {
            output.push(b' ');
        }
    }

    fn red(&self) -> &[u8] {
        if self.use_color { RED } else { b"" }
    }
    fn green(&self) -> &[u8] {
        if self.use_color { GREEN } else { b"" }
    }
    fn yellow(&self) -> &[u8] {
        if self.use_color { YELLOW } else { b"" }
    }
    fn cyan(&self) -> &[u8] {
        if self.use_color { CYAN } else { b"" }
    }
    fn dim(&self) -> &[u8] {
        if self.use_color { DIM } else { b"" }
    }
    fn bold(&self) -> &[u8] {
        if self.use_color { BOLD } else { b"" }
    }
    fn reset(&self) -> &[u8] {
        if self.use_color { RESET } else { b"" }
    }
}

/// Split on path/version separators so store-path hashes and version
/// components become individual diff tokens. `similar::from_words` splits
/// only on whitespace, which treats an entire store path as one token.
fn tokenize_path(s: &[u8]) -> Vec<&[u8]> {
    let mut toks = Vec::new();
    let mut start = 0;
    for (i, &b) in s.iter().enumerate() {
        if matches!(b, b'/' | b'-' | b'.' | b'_' | b':' | b' ' | b'\t') {
            if start < i {
                toks.push(&s[start..i]);
            }
            toks.push(&s[i..i + 1]);
            start = i + 1;
        }
    }
    if start < s.len() {
        toks.push(&s[start..]);
    }
    toks
}

/// An output change that only touches the store path (not hash/algo) is a
/// mechanical consequence of any other change and carries no information.
fn is_path_only_change(d: &OutputDetailDiff) -> bool {
    matches!(
        d,
        OutputDetailDiff::Changed {
            hash_algo: None,
            hash: None,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_drv() -> Derivation {
        Derivation {
            outputs: Default::default(),
            input_sources: Default::default(),
            input_derivations: Default::default(),
            platform: Vec::new(),
            builder: Vec::new(),
            args: Vec::new(),
            env: Default::default(),
        }
    }

    #[test]
    fn input_diff_shows_both_outputs_and_derivation() {
        // InputDiff.outputs describes which outputs are *consumed from* the
        // input (e.g., ["out"] -> ["out","dev"]). This is independent of the
        // nested derivation diff and must be shown even when both are set.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            ..Default::default()
        });
        let inner = DerivationDiff {
            original: empty_drv(),
            new: empty_drv(),
            outputs: OutputsDiff::Identical,
            platform: Some(StringDiff {
                old: b"x86_64-linux".to_vec(),
                new: b"aarch64-linux".to_vec(),
            }),
            builder: None,
            args: None,
            sources: None,
            inputs: None,
            env: None,
        };
        let inputs = InputsDiff {
            added: Default::default(),
            removed: Default::default(),
            changed: vec![InputDiff {
                path: b"foo.drv".to_vec(),
                outputs: Some(OutputSetDiff {
                    added: [b"dev".to_vec()].into(),
                    removed: Default::default(),
                }),
                derivation: Some(Box::new(inner)),
            }],
        };

        let mut out = Vec::new();
        renderer.format_inputs_diff(&mut out, &inputs, 0, 0);
        let out = String::from_utf8(out).unwrap();

        assert!(out.contains("aarch64-linux"), "nested drv diff missing");
        assert!(
            out.contains("dev"),
            "consumed-output change was swallowed:\n{out}"
        );
    }

    #[test]
    fn already_compared_input_is_labeled() {
        // When the cycle detector short-circuits a nested diff, the output
        // should say "already compared" rather than printing a dangling
        // "X differs" header with no body.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            ..Default::default()
        });
        let inner = DerivationDiff {
            original: empty_drv(),
            new: empty_drv(),
            outputs: OutputsDiff::AlreadyCompared,
            platform: None,
            builder: None,
            args: None,
            sources: None,
            inputs: None,
            env: None,
        };
        let inputs = InputsDiff {
            added: Default::default(),
            removed: Default::default(),
            changed: vec![InputDiff {
                path: b"foo.drv".to_vec(),
                outputs: None,
                derivation: Some(Box::new(inner)),
            }],
        };

        let mut out = Vec::new();
        renderer.format_inputs_diff(&mut out, &inputs, 0, 0);
        let out = String::from_utf8(out).unwrap();

        assert!(out.contains("foo.drv"));
        assert!(
            out.contains("already compared"),
            "expected 'already compared' marker, got:\n{out}"
        );
    }

    fn drv_with_output(name: &[u8], path: &[u8]) -> Derivation {
        let mut outputs = std::collections::BTreeMap::new();
        outputs.insert(
            name.to_vec(),
            Output {
                path: path.to_vec(),
                hash_algorithm: None,
                hash: None,
            },
        );
        Derivation {
            outputs,
            ..empty_drv()
        }
    }

    #[test]
    fn hides_output_path_noise_by_default() {
        // Output store paths differ whenever *anything* else differs. Showing
        // them on every nested derivation floods the diff with zero-signal
        // noise. The env var `$out` mirrors the same path and is equally
        // useless. Both must be hidden unless --verbose is set.
        let old = drv_with_output(b"out", b"/nix/store/aaa-foo");
        let new = drv_with_output(b"out", b"/nix/store/bbb-foo");
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            b"out".to_vec(),
            Some(EnvVarDiff::Changed(StringDiff {
                old: b"/nix/store/aaa-foo".to_vec(),
                new: b"/nix/store/bbb-foo".to_vec(),
            })),
        );
        env.insert(
            b"version".to_vec(),
            Some(EnvVarDiff::Changed(StringDiff {
                old: b"1".to_vec(),
                new: b"2".to_vec(),
            })),
        );
        let diff = DerivationDiff {
            original: old,
            new,
            outputs: OutputsDiff::Changed(vec![OutputDiff {
                name: b"out".to_vec(),
                diff: OutputDetailDiff::Changed {
                    old: Output {
                        path: b"/nix/store/aaa-foo".to_vec(),
                        hash_algorithm: None,
                        hash: None,
                    },
                    new: Box::new(Output {
                        path: b"/nix/store/bbb-foo".to_vec(),
                        hash_algorithm: None,
                        hash: None,
                    }),
                    path: Some(StringDiff {
                        old: b"/nix/store/aaa-foo".to_vec(),
                        new: b"/nix/store/bbb-foo".to_vec(),
                    }),
                    hash_algo: None,
                    hash: None,
                },
            }]),
            platform: None,
            builder: None,
            args: None,
            sources: None,
            inputs: None,
            env: Some(env),
        };

        let quiet = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            ..Default::default()
        });
        let out = String::from_utf8(quiet.format_derivation_diff(&diff, 0, 0)).unwrap();
        assert!(!out.contains("Outputs"), "path-only output shown:\n{out}");
        assert!(!out.contains("out:"), "$out env var shown:\n{out}");
        assert!(out.contains("version"), "real env change missing:\n{out}");

        let verbose = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            verbose: true,
            ..Default::default()
        });
        let out = String::from_utf8(verbose.format_derivation_diff(&diff, 0, 0)).unwrap();
        assert!(out.contains("Outputs"), "verbose should show outputs");
        assert!(out.contains("out:"), "verbose should show $out");
    }

    #[test]
    fn shows_fod_hash_changes() {
        // Fixed-output derivation hash changes are semantically meaningful
        // (e.g., a src update) and must NOT be filtered as path noise.
        let diff = OutputDetailDiff::Changed {
            old: Output {
                path: b"/nix/store/aaa-src".to_vec(),
                hash_algorithm: Some(b"sha256".to_vec()),
                hash: Some(b"old".to_vec()),
            },
            new: Box::new(Output {
                path: b"/nix/store/bbb-src".to_vec(),
                hash_algorithm: Some(b"sha256".to_vec()),
                hash: Some(b"new".to_vec()),
            }),
            path: Some(StringDiff {
                old: b"/nix/store/aaa-src".to_vec(),
                new: b"/nix/store/bbb-src".to_vec(),
            }),
            hash_algo: None,
            hash: Some(StringDiff {
                old: b"old".to_vec(),
                new: b"new".to_vec(),
            }),
        };
        assert!(!is_path_only_change(&diff));
    }

    #[test]
    fn truncates_large_input_lists() {
        // A stdenv bump can produce 100+ added/removed inputs. Listing them
        // all buries the interesting changes.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            input_list_limit: 3,
            ..Default::default()
        });
        let paths: Vec<Vec<u8>> = (0..10).map(|i| format!("path{i}").into_bytes()).collect();
        let mut out = Vec::new();
        renderer.write_path_list(&mut out, paths.iter(), b"+ ", b"", 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("path0"));
        assert!(out.contains("path2"));
        assert!(!out.contains("path3"), "should be truncated:\n{out}");
        assert!(out.contains("7 more"), "should summarize remainder:\n{out}");
    }

    #[test]
    fn format_text_diff_limits_context() {
        // With context_lines=1, only 1 context line should surround a change.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            context_lines: 1,
            ..Default::default()
        });
        let old = b"a\nb\nc\nd\ne\n";
        let new = b"a\nb\nNEW\nc\nd\ne\n";

        let mut out = Vec::new();
        renderer.format_text_diff(&mut out, old, new, 0);
        let out = String::from_utf8(out).unwrap();

        // Leading: only "b" (1 line before change), then NEW, then only "c"
        assert!(!out.contains("  a\n"), "leading context not limited: {out}");
        assert!(out.contains("  b\n"));
        assert!(out.contains("+ NEW\n"));
        assert!(out.contains("  c\n"));
        assert!(
            !out.contains("  d\n"),
            "trailing context not limited: {out}"
        );
        assert!(!out.contains("  e\n"));
    }

    #[test]
    fn inline_highlight_marks_changed_words() {
        // With inline highlighting on, only the changed word segments should
        // be wrapped in reverse-video, not the whole line. This lets the
        // reader spot store-path hash changes and version bumps at a glance.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Always,
            ..Default::default()
        });
        let diff = StringDiff {
            old: b"/nix/store/aaa-foo-1.0".to_vec(),
            new: b"/nix/store/bbb-foo-2.0".to_vec(),
        };
        let mut out = Vec::new();
        renderer.format_string_diff(&mut out, &diff, 0);
        let out = String::from_utf8(out).unwrap();

        // "foo" is unchanged → must NOT be inside a reverse-video span.
        assert!(
            out.contains("\x1b[7maaa\x1b[27m"),
            "hash not highlighted:\n{out:?}"
        );
        assert!(
            out.contains("\x1b[7mbbb\x1b[27m"),
            "hash not highlighted:\n{out:?}"
        );
        // The common prefix "/nix/store/" must appear outside reverse-video.
        assert!(
            out.contains("- /nix/store/\x1b[7m"),
            "common prefix wrongly highlighted:\n{out:?}"
        );
    }

    #[test]
    fn inline_highlight_disabled_without_color() {
        // Reverse-video escapes are meaningless without color; inline
        // highlighting must auto-disable to avoid emitting them.
        let renderer = Renderer::new(RenderOptions {
            color_mode: ColorMode::Never,
            inline_highlight: true,
            ..Default::default()
        });
        assert!(!renderer.inline_highlight);
    }
}
