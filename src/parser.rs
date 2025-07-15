use crate::types::{Derivation, Output, StorePath};
use anyhow::{anyhow, Context, Result};
use memchr::{memchr, memchr2};
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
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            input,
            bytes: input.as_bytes(),
            pos: 0,
        }
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
        let mut outputs = BTreeMap::new(); // BTreeMap doesn't support with_capacity

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

        // Find the closing quote position
        let start = self.pos;
        let end_pos =
            memchr(b'"', &self.bytes[start..]).ok_or_else(|| anyhow!("Unterminated string"))?;

        // Fast path: check if there are any escapes in the string
        if memchr(b'\\', &self.bytes[start..start + end_pos]).is_none() {
            // No escapes, we can just copy the bytes directly
            let s = std::str::from_utf8(&self.bytes[start..start + end_pos])
                .map_err(|e| anyhow!("Invalid UTF-8 in string: {}", e))?;
            self.pos = start + end_pos + 1; // Skip past the closing quote
            return Ok(s.to_string());
        }

        // Slow path: handle escapes using SIMD to find next escape or quote
        let mut result = String::with_capacity(end_pos); // Use actual string length as hint
        let mut current_pos = self.pos;

        loop {
            // Find next quote or backslash using SIMD
            if let Some(special_pos) = memchr2(b'"', b'\\', &self.bytes[current_pos..]) {
                // Copy everything before the special character
                if special_pos > 0 {
                    let chunk = std::str::from_utf8(&self.bytes[current_pos..current_pos + special_pos])
                        .map_err(|e| anyhow!("Invalid UTF-8 in string: {}", e))?;
                    result.push_str(chunk);
                }
                
                current_pos += special_pos;
                let special_char = self.bytes[current_pos];
                
                if special_char == b'"' {
                    // Found closing quote
                    self.pos = current_pos + 1;
                    return Ok(result);
                } else {
                    // Handle escape
                    current_pos += 1; // Skip backslash
                    if current_pos >= self.bytes.len() {
                        return Err(anyhow!("Unexpected end of input in string"));
                    }
                    
                    let escaped = self.bytes[current_pos];
                    match escaped {
                        b'n' => result.push('\n'),
                        b't' => result.push('\t'),
                        b'r' => result.push('\r'),
                        b'\\' => result.push('\\'),
                        b'"' => result.push('"'),
                        _ => {
                            // For non-ASCII or other escapes, need to handle UTF-8
                            if escaped < 128 {
                                result.push(escaped as char);
                            } else {
                                // Get the full UTF-8 character
                                let ch = self.input[current_pos..].chars().next()
                                    .ok_or_else(|| anyhow!("Invalid escape sequence"))?;
                                result.push(ch);
                                current_pos += ch.len_utf8() - 1; // -1 because we'll increment below
                            }
                        }
                    }
                    current_pos += 1;
                }
            } else {
                // No more quotes or escapes, this is an error
                return Err(anyhow!("Unterminated string"));
            }
        }
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
        if self.pos >= self.bytes.len() {
            return None;
        }
        // Fast path for ASCII
        let byte = self.bytes[self.pos];
        if byte < 128 {
            return Some(byte as char);
        }
        // Slower path for UTF-8
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if self.pos >= self.bytes.len() {
            return;
        }
        // Fast path for ASCII
        if self.bytes[self.pos] < 128 {
            self.pos += 1;
        } else if let Some(ch) = self.input[self.pos..].chars().next() {
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
