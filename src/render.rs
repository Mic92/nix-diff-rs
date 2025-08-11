use crate::types::*;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::io::{self, IsTerminal, Write};

const RED: &[u8] = b"\x1b[31m";
const GREEN: &[u8] = b"\x1b[32m";
const YELLOW: &[u8] = b"\x1b[33m";
#[allow(dead_code)]
const BLUE: &[u8] = b"\x1b[34m";
#[allow(dead_code)]
const MAGENTA: &[u8] = b"\x1b[35m";
#[allow(dead_code)]
const CYAN: &[u8] = b"\x1b[36m";
const BOLD: &[u8] = b"\x1b[1m";
const RESET: &[u8] = b"\x1b[0m";

pub struct Renderer {
    color_mode: ColorMode,
    context_lines: usize,
}

impl Renderer {
    pub fn new(color_mode: ColorMode, context_lines: usize) -> Self {
        Renderer {
            color_mode,
            context_lines,
        }
    }

    pub fn render(&self, diff: &DerivationDiff) -> io::Result<()> {
        let mut stdout = io::stdout();
        let output = self.format_derivation_diff(diff, 0);
        if output.is_empty() {
            stdout.write_all(b"The derivations are identical.\n")?;
        } else {
            stdout.write_all(&output)?;
        }
        stdout.flush()
    }

    fn format_derivation_diff(&self, diff: &DerivationDiff, indent: usize) -> Vec<u8> {
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

        if let OutputsDiff::Changed(output_diffs) = outputs {
            self.write_section(&mut output, b"Outputs", indent);
            for out_diff in output_diffs {
                self.format_output_diff(&mut output, out_diff, indent + 2);
            }
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
            for (i, arg_diff) in arg_diffs.iter().enumerate() {
                self.write_indent(&mut output, indent + 2);
                output.extend_from_slice(b"Argument ");
                output.extend_from_slice(i.to_string().as_bytes());
                output.extend_from_slice(b":\n");
                // For multi-line arguments (like scripts), show them as a text diff
                let StringDiff { old, new } = arg_diff;
                if old.contains(&b'\n') || new.contains(&b'\n') {
                    // Create a proper line-by-line diff
                    let text_diff = self.create_text_diff(old, new);
                    self.format_text_diff(&mut output, &text_diff, indent + 4);
                } else {
                    self.format_string_diff(&mut output, arg_diff, indent + 4);
                }
            }
        }

        if let Some(src_diff) = sources {
            self.format_sources_diff(&mut output, src_diff, indent);
        }

        if let Some(inp_diff) = inputs {
            self.format_inputs_diff(&mut output, inp_diff, indent);
        }

        if let Some(env_diffs) = env {
            self.write_section(&mut output, b"Environment", indent);
            for (key, var_diff) in env_diffs {
                if let Some(diff) = var_diff {
                    self.write_indent(&mut output, indent + 2);
                    output.extend_from_slice(key);
                    output.extend_from_slice(b":\n");
                    self.format_env_var_diff(&mut output, diff, indent + 4);
                }
            }
        }

        output
    }

    fn format_output_diff(&self, output: &mut Vec<u8>, diff: &OutputDiff, indent: usize) {
        self.write_indent(output, indent);
        output.extend_from_slice(b"Output '");
        output.extend_from_slice(&diff.name);
        output.extend_from_slice(b"':\n");

        match &diff.diff {
            OutputDetailDiff::Added(out) => {
                self.write_indent(output, indent + 2);
                output.extend_from_slice(self.green());
                output.extend_from_slice(b"+ Added: ");
                output.extend_from_slice(&out.path);
                output.extend_from_slice(self.reset());
                output.push(b'\n');
            }
            OutputDetailDiff::Removed(out) => {
                self.write_indent(output, indent + 2);
                output.extend_from_slice(self.red());
                output.extend_from_slice(b"- Removed: ");
                output.extend_from_slice(&out.path);
                output.extend_from_slice(self.reset());
                output.push(b'\n');
            }
            OutputDetailDiff::Changed {
                path,
                hash_algo,
                hash,
                ..
            } => {
                if let Some(path_diff) = path {
                    self.write_indent(output, indent + 2);
                    output.extend_from_slice(b"Path:\n");
                    self.format_string_diff(output, path_diff, indent + 4);
                }
                if let Some(algo_diff) = hash_algo {
                    self.write_indent(output, indent + 2);
                    output.extend_from_slice(b"Hash algorithm:\n");
                    self.format_string_diff(output, algo_diff, indent + 4);
                }
                if let Some(hash_diff) = hash {
                    self.write_indent(output, indent + 2);
                    output.extend_from_slice(b"Hash:\n");
                    self.format_string_diff(output, hash_diff, indent + 4);
                }
            }
        }
    }

    fn format_string_diff(&self, output: &mut Vec<u8>, diff: &StringDiff, indent: usize) {
        let StringDiff { old, new } = diff;
        self.write_indent(output, indent);
        output.extend_from_slice(self.red());
        output.extend_from_slice(b"- ");
        output.extend_from_slice(old);
        output.extend_from_slice(self.reset());
        output.push(b'\n');

        self.write_indent(output, indent);
        output.extend_from_slice(self.green());
        output.extend_from_slice(b"+ ");
        output.extend_from_slice(new);
        output.extend_from_slice(self.reset());
        output.push(b'\n');
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
            output.extend_from_slice(self.red());
            output.extend_from_slice(b"- ");
            output.extend_from_slice(path);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }

        for path in added {
            self.write_indent(output, indent + 2);
            output.extend_from_slice(self.green());
            output.extend_from_slice(b"+ ");
            output.extend_from_slice(path);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }

        for src_diff in common {
            self.write_indent(output, indent + 2);
            output.extend_from_slice(self.yellow());
            output.extend_from_slice(b"~ ");
            output.extend_from_slice(&src_diff.path);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
            self.format_text_diff(output, &src_diff.diff, indent + 4);
        }
    }

    fn format_inputs_diff(&self, output: &mut Vec<u8>, diff: &InputsDiff, indent: usize) {
        let InputsDiff {
            added,
            removed,
            changed,
        } = diff;
        self.write_section(output, b"Input derivations", indent);

        for path in removed {
            self.write_indent(output, indent + 2);
            output.extend_from_slice(self.red());
            output.extend_from_slice(b"- ");
            output.extend_from_slice(&path.0);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }

        for path in added {
            self.write_indent(output, indent + 2);
            output.extend_from_slice(self.green());
            output.extend_from_slice(b"+ ");
            output.extend_from_slice(&path.0);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }

        for inp_diff in changed {
            self.write_indent(output, indent + 2);
            output.extend_from_slice(self.yellow());
            output.extend_from_slice(b"~ ");
            output.extend_from_slice(&inp_diff.path);
            output.extend_from_slice(self.reset());
            output.push(b'\n');

            if let Some(out_diff) = &inp_diff.outputs {
                self.write_indent(output, indent + 4);
                output.extend_from_slice(b"Output changes:\n");
                self.format_output_set_diff(output, out_diff, indent + 6);
            }

            if let Some(drv_diff) = &inp_diff.derivation {
                let sub_output = self.format_derivation_diff(drv_diff, indent + 4);
                output.extend_from_slice(&sub_output);
            }
        }
    }

    fn format_output_set_diff(&self, output: &mut Vec<u8>, diff: &OutputSetDiff, indent: usize) {
        let OutputSetDiff { added, removed } = diff;
        for out in removed {
            self.write_indent(output, indent);
            output.extend_from_slice(self.red());
            output.extend_from_slice(b"- ");
            output.extend_from_slice(out);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }
        for out in added {
            self.write_indent(output, indent);
            output.extend_from_slice(self.green());
            output.extend_from_slice(b"+ ");
            output.extend_from_slice(out);
            output.extend_from_slice(self.reset());
            output.push(b'\n');
        }
    }

    fn format_env_var_diff(&self, output: &mut Vec<u8>, diff: &EnvVarDiff, indent: usize) {
        match diff {
            EnvVarDiff::Added(value) => {
                self.write_indent(output, indent);
                output.extend_from_slice(self.green());
                output.extend_from_slice(b"+ ");
                output.extend_from_slice(value);
                output.extend_from_slice(self.reset());
                output.push(b'\n');
            }
            EnvVarDiff::Removed(value) => {
                self.write_indent(output, indent);
                output.extend_from_slice(self.red());
                output.extend_from_slice(b"- ");
                output.extend_from_slice(value);
                output.extend_from_slice(self.reset());
                output.push(b'\n');
            }
            EnvVarDiff::Changed(str_diff) => {
                self.format_string_diff(output, str_diff, indent);
            }
        }
    }

    fn format_text_diff(&self, output: &mut Vec<u8>, diff: &TextDiff, indent: usize) {
        match diff {
            TextDiff::Binary => {
                self.write_indent(output, indent);
                output.extend_from_slice(self.yellow());
                output.extend_from_slice(b"Binary files differ");
                output.extend_from_slice(self.reset());
                output.push(b'\n');
            }
            TextDiff::Text(lines) => {
                let mut context_count = 0;
                let mut in_change_block = false;

                for line in lines {
                    match line {
                        DiffLine::Context(text) => {
                            if in_change_block || context_count < self.context_lines {
                                self.write_indent(output, indent);
                                output.extend_from_slice(b"  ");
                                output.extend_from_slice(text);
                                context_count += 1;
                            }
                        }
                        DiffLine::Added(text) => {
                            self.write_indent(output, indent);
                            output.extend_from_slice(self.green());
                            output.extend_from_slice(b"+ ");
                            output.extend_from_slice(text);
                            output.extend_from_slice(self.reset());
                            in_change_block = true;
                            context_count = 0;
                        }
                        DiffLine::Removed(text) => {
                            self.write_indent(output, indent);
                            output.extend_from_slice(self.red());
                            output.extend_from_slice(b"- ");
                            output.extend_from_slice(text);
                            output.extend_from_slice(self.reset());
                            in_change_block = true;
                            context_count = 0;
                        }
                    }
                }
            }
        }
    }

    fn write_section(&self, output: &mut Vec<u8>, title: &[u8], indent: usize) {
        self.write_indent(output, indent);
        output.extend_from_slice(self.bold());
        output.extend_from_slice(title);
        output.extend_from_slice(b":");
        output.extend_from_slice(self.reset());
        output.push(b'\n');
    }

    fn write_indent(&self, output: &mut Vec<u8>, indent: usize) {
        for _ in 0..indent {
            output.push(b' ');
        }
    }

    fn should_use_color(&self) -> bool {
        // Check NO_COLOR environment variable
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        match self.color_mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => io::stdout().is_terminal(),
        }
    }

    fn red(&self) -> &[u8] {
        if self.should_use_color() { RED } else { b"" }
    }
    fn green(&self) -> &[u8] {
        if self.should_use_color() { GREEN } else { b"" }
    }
    fn yellow(&self) -> &[u8] {
        if self.should_use_color() { YELLOW } else { b"" }
    }
    fn bold(&self) -> &[u8] {
        if self.should_use_color() { BOLD } else { b"" }
    }
    fn reset(&self) -> &[u8] {
        if self.should_use_color() { RESET } else { b"" }
    }

    fn create_text_diff(&self, old: &[u8], new: &[u8]) -> TextDiff {
        // Use similar's TextDiff to create a line-by-line diff
        let diff = SimilarTextDiff::from_lines(old, new);

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
