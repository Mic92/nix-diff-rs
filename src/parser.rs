use crate::types::{Derivation, Output, StorePath};
use anyhow::{anyhow, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn parse_derivation(path: &Path) -> Result<Derivation> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read derivation file: {}", path.display()))?;

    parse_derivation_string(&content)
}

pub fn parse_derivation_string(input: &str) -> Result<Derivation> {
    let mut parser = Parser::new(input);
    parser.parse_derivation()
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser { input, pos: 0 }
    }

    fn parse_derivation(&mut self) -> Result<Derivation> {
        self.expect_str("Derive(")?;

        // Parse outputs
        let outputs = self.parse_outputs()?;
        self.expect_char(',')?;

        // Parse input derivations
        let input_derivations = self.parse_input_derivations()?;
        self.expect_char(',')?;

        // Parse input sources
        let input_sources = self.parse_input_sources()?;
        self.expect_char(',')?;

        // Parse platform
        let platform = self.parse_string()?;
        self.expect_char(',')?;

        // Parse builder
        let builder_path = self.parse_string()?;
        let builder = StorePath::new(PathBuf::from(builder_path));
        self.expect_char(',')?;

        // Parse args
        let args = self.parse_string_list()?;
        self.expect_char(',')?;

        // Parse environment
        let env = self.parse_environment()?;

        self.expect_char(')')?;

        Ok(Derivation {
            outputs,
            input_sources,
            input_derivations,
            platform,
            builder,
            args,
            env,
        })
    }

    fn parse_outputs(&mut self) -> Result<BTreeMap<String, Output>> {
        self.expect_char('[')?;
        let mut outputs = BTreeMap::new();

        while self.peek() != Some(']') {
            self.expect_char('(')?;
            let name = self.parse_string()?;
            self.expect_char(',')?;
            let path = StorePath::new(PathBuf::from(self.parse_string()?));
            self.expect_char(',')?;
            let hash_algorithm = self.parse_optional_string()?;
            self.expect_char(',')?;
            let hash = self.parse_optional_string()?;
            self.expect_char(')')?;

            outputs.insert(
                name,
                Output {
                    path,
                    hash_algorithm,
                    hash,
                },
            );

            if self.peek() == Some(',') {
                self.advance();
            }
        }

        self.expect_char(']')?;
        Ok(outputs)
    }

    fn parse_input_derivations(&mut self) -> Result<BTreeMap<StorePath, BTreeSet<String>>> {
        self.expect_char('[')?;
        let mut inputs = BTreeMap::new();

        while self.peek() != Some(']') {
            self.expect_char('(')?;
            let path = StorePath::new(PathBuf::from(self.parse_string()?));
            self.expect_char(',')?;
            let outputs = self.parse_string_set()?;
            self.expect_char(')')?;

            inputs.insert(path, outputs);

            if self.peek() == Some(',') {
                self.advance();
            }
        }

        self.expect_char(']')?;
        Ok(inputs)
    }

    fn parse_input_sources(&mut self) -> Result<BTreeSet<StorePath>> {
        let paths = self.parse_string_list()?;
        Ok(paths
            .into_iter()
            .map(|p| StorePath::new(PathBuf::from(p)))
            .collect())
    }

    fn parse_environment(&mut self) -> Result<BTreeMap<String, String>> {
        self.expect_char('[')?;
        let mut env = BTreeMap::new();

        while self.peek() != Some(']') {
            self.expect_char('(')?;
            let key = self.parse_string()?;
            self.expect_char(',')?;
            let value = self.parse_string()?;
            self.expect_char(')')?;

            env.insert(key, value);

            if self.peek() == Some(',') {
                self.advance();
            }
        }

        self.expect_char(']')?;
        Ok(env)
    }

    fn parse_string(&mut self) -> Result<String> {
        self.skip_whitespace();
        self.expect_char('"')?;
        let mut result = String::new();

        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Ok(result);
            } else if ch == '\\' {
                self.advance();
                match self.peek() {
                    Some('n') => {
                        self.advance();
                        result.push('\n');
                    }
                    Some('t') => {
                        self.advance();
                        result.push('\t');
                    }
                    Some('r') => {
                        self.advance();
                        result.push('\r');
                    }
                    Some('\\') => {
                        self.advance();
                        result.push('\\');
                    }
                    Some('"') => {
                        self.advance();
                        result.push('"');
                    }
                    Some(c) => {
                        self.advance();
                        result.push(c);
                    }
                    None => return Err(anyhow!("Unexpected end of input in string")),
                }
            } else {
                result.push(ch);
                self.advance();
            }
        }

        Err(anyhow!("Unterminated string"))
    }

    fn parse_optional_string(&mut self) -> Result<Option<String>> {
        self.skip_whitespace();
        if self.peek() == Some('"') {
            Ok(Some(self.parse_string()?))
        } else {
            self.expect_str("")?;
            Ok(None)
        }
    }

    fn parse_string_list(&mut self) -> Result<Vec<String>> {
        self.expect_char('[')?;
        let mut items = Vec::new();

        while self.peek() != Some(']') {
            items.push(self.parse_string()?);
            if self.peek() == Some(',') {
                self.advance();
            }
        }

        self.expect_char(']')?;
        Ok(items)
    }

    fn parse_string_set(&mut self) -> Result<BTreeSet<String>> {
        self.expect_char('[')?;
        let mut items = BTreeSet::new();

        while self.peek() != Some(']') {
            items.insert(self.parse_string()?);
            if self.peek() == Some(',') {
                self.advance();
            }
        }

        self.expect_char(']')?;
        Ok(items)
    }

    fn expect_str(&mut self, expected: &str) -> Result<()> {
        self.skip_whitespace();
        if self.input[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            Ok(())
        } else {
            Err(anyhow!("Expected '{}' at position {}", expected, self.pos))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        self.skip_whitespace();
        match self.peek() {
            Some(ch) if ch == expected => {
                self.advance();
                Ok(())
            }
            Some(ch) => Err(anyhow!(
                "Expected '{}' but found '{}' at position {}",
                expected,
                ch,
                self.pos
            )),
            None => Err(anyhow!("Expected '{}' but reached end of input", expected)),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.chars().nth(self.pos)
    }

    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.pos += ch.len_utf8();
        }
    }
}

pub fn get_derivation_path(store_path: &Path) -> Result<PathBuf> {
    // If it's already a .drv file, return it
    if store_path.extension().and_then(|s| s.to_str()) == Some("drv") {
        return Ok(store_path.to_path_buf());
    }

    // Otherwise, query the derivation
    let output = std::process::Command::new("nix-store")
        .arg("--query")
        .arg("--deriver")
        .arg(store_path)
        .output()
        .with_context(|| {
            format!(
                "Failed to run nix-store --query --deriver for path: {}",
                store_path.display()
            )
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to query derivation for {}: {}",
            store_path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let drv_path = String::from_utf8(output.stdout)?.trim().to_string();

    Ok(PathBuf::from(drv_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_derivation() {
        let drv = r#"Derive([("out","/nix/store/abc-test","","")],[],[],"/bin/bash","/nix/store/xyz-builder",["-c","echo hello"],[("name","test"),("out","/nix/store/abc-test")])"#;
        let result = parse_derivation_string(drv).unwrap();
        assert_eq!(result.outputs.len(), 1);
        assert_eq!(result.platform, "/bin/bash");
        assert_eq!(result.args, vec!["-c", "echo hello"]);
    }
}
