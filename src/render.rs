use crate::types::*;
use similar::{ChangeTag, TextDiff as SimilarTextDiff};
use std::fmt::Write as FmtWrite;
use std::io::{self, Write};

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
#[allow(dead_code)]
const BLUE: &str = "\x1b[34m";
#[allow(dead_code)]
const MAGENTA: &str = "\x1b[35m";
#[allow(dead_code)]
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

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
            writeln!(stdout, "The derivations are identical.")?;
        } else {
            write!(stdout, "{output}")?;
        }
        stdout.flush()
    }

    fn format_derivation_diff(&self, diff: &DerivationDiff, indent: usize) -> String {
        let mut output = String::new();

        match diff {
            DerivationDiff::Changed {
                outputs,
                platform,
                builder,
                args,
                sources,
                inputs,
                env,
                ..
            } => {
                if let OutputsDiff::Changed(output_diffs) = outputs {
                    self.write_section(&mut output, "Outputs", indent);
                    for out_diff in output_diffs {
                        self.format_output_diff(&mut output, out_diff, indent + 2);
                    }
                }

                if let Some(plat_diff) = platform {
                    self.write_section(&mut output, "Platform", indent);
                    self.format_string_diff(&mut output, plat_diff, indent + 2);
                }

                if let Some(builder_diff) = builder {
                    self.write_section(&mut output, "Builder", indent);
                    self.format_string_diff(&mut output, builder_diff, indent + 2);
                }

                if let Some(ArgumentsDiff::Changed(arg_diffs)) = args {
                    self.write_section(&mut output, "Arguments", indent);
                    for (i, arg_diff) in arg_diffs.iter().enumerate() {
                        self.write_indent(&mut output, indent + 2);
                        let _ = writeln!(&mut output, "Argument {i}:");
                        // For multi-line arguments (like scripts), use text diff
                        if let StringDiff::Changed { old, new } = arg_diff {
                            if old.contains('\n') || new.contains('\n') {
                                let text_diff = self.create_text_diff(old, new);
                                self.format_text_diff(&mut output, &text_diff, indent + 4);
                            } else {
                                self.format_string_diff(&mut output, arg_diff, indent + 4);
                            }
                        }
                    }
                }

                if let Some(src_diff) = sources {
                    self.format_sources_diff(&mut output, src_diff, indent);
                }

                if let Some(inp_diff) = inputs {
                    self.format_inputs_diff(&mut output, inp_diff, indent);
                }

                if let Some(EnvironmentDiff::Changed(env_diffs)) = env {
                    self.write_section(&mut output, "Environment", indent);
                    for (key, var_diff) in env_diffs {
                        if let Some(diff) = var_diff {
                            self.write_indent(&mut output, indent + 2);
                            let _ = writeln!(&mut output, "{key}:");
                            self.format_env_var_diff(&mut output, diff, indent + 4);
                        }
                    }
                }
            }
        }

        output
    }

    fn format_output_diff(&self, output: &mut String, diff: &OutputDiff, indent: usize) {
        self.write_indent(output, indent);
        let _ = writeln!(output, "Output '{}':", diff.name);

        match &diff.diff {
            OutputDetailDiff::Added(out) => {
                self.write_indent(output, indent + 2);
                let _ = writeln!(
                    output,
                    "{}+ Added: {}{}",
                    self.green(),
                    out.path.path_str,
                    self.reset()
                );
            }
            OutputDetailDiff::Removed(out) => {
                self.write_indent(output, indent + 2);
                let _ = writeln!(
                    output,
                    "{}- Removed: {}{}",
                    self.red(),
                    out.path.path_str,
                    self.reset()
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
                    let _ = writeln!(output, "Path:");
                    self.format_string_diff(output, path_diff, indent + 4);
                }
                if let Some(algo_diff) = hash_algo {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(output, "Hash algorithm:");
                    self.format_string_diff(output, algo_diff, indent + 4);
                }
                if let Some(hash_diff) = hash {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(output, "Hash:");
                    self.format_string_diff(output, hash_diff, indent + 4);
                }
            }
        }
    }

    fn format_string_diff(&self, output: &mut String, diff: &StringDiff, indent: usize) {
        match diff {
            StringDiff::Identical => {}
            StringDiff::Changed { old, new } => {
                self.write_indent(output, indent);
                let _ = writeln!(output, "{}- {}{}", self.red(), old, self.reset());
                self.write_indent(output, indent);
                let _ = writeln!(output, "{}+ {}{}", self.green(), new, self.reset());
            }
        }
    }

    fn format_sources_diff(&self, output: &mut String, diff: &SourcesDiff, indent: usize) {
        match diff {
            SourcesDiff::Identical => {}
            SourcesDiff::Changed {
                added,
                removed,
                common,
            } => {
                self.write_section(output, "Sources", indent);

                for path in removed {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(output, "{}- {}{}", self.red(), path.path_str, self.reset());
                }

                for path in added {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(
                        output,
                        "{}+ {}{}",
                        self.green(),
                        path.path_str,
                        self.reset()
                    );
                }

                for src_diff in common {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(
                        output,
                        "{}~ {}{}",
                        self.yellow(),
                        src_diff.path.path_str,
                        self.reset()
                    );
                    self.format_text_diff(output, &src_diff.diff, indent + 4);
                }
            }
        }
    }

    fn format_inputs_diff(&self, output: &mut String, diff: &InputsDiff, indent: usize) {
        match diff {
            InputsDiff::Identical => {}
            InputsDiff::Changed {
                added,
                removed,
                changed,
            } => {
                self.write_section(output, "Input derivations", indent);

                for path in removed {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(output, "{}- {}{}", self.red(), path.path_str, self.reset());
                }

                for path in added {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(
                        output,
                        "{}+ {}{}",
                        self.green(),
                        path.path_str,
                        self.reset()
                    );
                }

                for inp_diff in changed {
                    self.write_indent(output, indent + 2);
                    let _ = writeln!(
                        output,
                        "{}~ {}{}",
                        self.yellow(),
                        inp_diff.path.path_str,
                        self.reset()
                    );

                    if let Some(out_diff) = &inp_diff.outputs {
                        self.write_indent(output, indent + 4);
                        let _ = writeln!(output, "Output changes:");
                        self.format_output_set_diff(output, out_diff, indent + 6);
                    }

                    if let Some(drv_diff) = &inp_diff.derivation {
                        let _ = write!(
                            output,
                            "{}",
                            self.format_derivation_diff(drv_diff, indent + 4)
                        );
                    }
                }
            }
        }
    }

    fn format_output_set_diff(&self, output: &mut String, diff: &OutputSetDiff, indent: usize) {
        match diff {
            OutputSetDiff::Added(outputs) => {
                for out in outputs {
                    self.write_indent(output, indent);
                    let _ = writeln!(output, "{}+ {}{}", self.green(), out, self.reset());
                }
            }
            OutputSetDiff::Removed(outputs) => {
                for out in outputs {
                    self.write_indent(output, indent);
                    let _ = writeln!(output, "{}- {}{}", self.red(), out, self.reset());
                }
            }
            OutputSetDiff::Changed { added, removed } => {
                for out in removed {
                    self.write_indent(output, indent);
                    let _ = writeln!(output, "{}- {}{}", self.red(), out, self.reset());
                }
                for out in added {
                    self.write_indent(output, indent);
                    let _ = writeln!(output, "{}+ {}{}", self.green(), out, self.reset());
                }
            }
        }
    }

    fn format_env_var_diff(&self, output: &mut String, diff: &EnvVarDiff, indent: usize) {
        match diff {
            EnvVarDiff::Added(value) => {
                self.write_indent(output, indent);
                let _ = writeln!(output, "{}+ {}{}", self.green(), value, self.reset());
            }
            EnvVarDiff::Removed(value) => {
                self.write_indent(output, indent);
                let _ = writeln!(output, "{}- {}{}", self.red(), value, self.reset());
            }
            EnvVarDiff::Changed(str_diff) => {
                self.format_string_diff(output, str_diff, indent);
            }
        }
    }

    fn format_text_diff(&self, output: &mut String, diff: &TextDiff, indent: usize) {
        match diff {
            TextDiff::Binary => {
                self.write_indent(output, indent);
                let _ = writeln!(
                    output,
                    "{}Binary files differ{}",
                    self.yellow(),
                    self.reset()
                );
            }
            TextDiff::Text(lines) => {
                let mut context_count = 0;
                let mut in_change_block = false;

                for line in lines {
                    match line {
                        DiffLine::Context(text) => {
                            if in_change_block || context_count < self.context_lines {
                                self.write_indent(output, indent);
                                let _ = write!(output, "  {text}");
                                context_count += 1;
                            }
                        }
                        DiffLine::Added(text) => {
                            self.write_indent(output, indent);
                            let _ = write!(output, "{}+ {}{}", self.green(), text, self.reset());
                            in_change_block = true;
                            context_count = 0;
                        }
                        DiffLine::Removed(text) => {
                            self.write_indent(output, indent);
                            let _ = write!(output, "{}- {}{}", self.red(), text, self.reset());
                            in_change_block = true;
                            context_count = 0;
                        }
                    }
                }
            }
        }
    }

    fn write_section(&self, output: &mut String, title: &str, indent: usize) {
        self.write_indent(output, indent);
        let _ = writeln!(output, "{}{}:{}", self.bold(), title, self.reset());
    }

    fn write_indent(&self, output: &mut String, indent: usize) {
        for _ in 0..indent {
            output.push(' ');
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
            ColorMode::Auto => atty::is(atty::Stream::Stdout),
        }
    }

    fn red(&self) -> &str {
        if self.should_use_color() {
            RED
        } else {
            ""
        }
    }
    fn green(&self) -> &str {
        if self.should_use_color() {
            GREEN
        } else {
            ""
        }
    }
    fn yellow(&self) -> &str {
        if self.should_use_color() {
            YELLOW
        } else {
            ""
        }
    }
    fn bold(&self) -> &str {
        if self.should_use_color() {
            BOLD
        } else {
            ""
        }
    }
    fn reset(&self) -> &str {
        if self.should_use_color() {
            RESET
        } else {
            ""
        }
    }

    fn create_text_diff(&self, old: &str, new: &str) -> TextDiff {
        let diff = match DiffOrientation::Line {
            DiffOrientation::Line => SimilarTextDiff::from_lines(old, new),
            DiffOrientation::Word => SimilarTextDiff::from_words(old, new),
            DiffOrientation::Character => SimilarTextDiff::from_chars(old, new),
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
