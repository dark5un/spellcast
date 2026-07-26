// SPDX-License-Identifier: Apache-2.0

//! Tree-sitter backed tokenizer.
//!
//! Replaces the heuristic tokenizer with language-aware parsing.
//! Detects language by file extension, shebang lines, or syntax
//! patterns, then uses the appropriate tree-sitter grammar to
//! produce typed tokens.

use std::collections::HashMap;

use tree_sitter::{Node, Parser, Tree};

use crate::error::SpellcastError;
use crate::error::SpellcastResult;
use crate::tokenizer::{Token, TokenContext, TokenStream, TokenType, Tokenizer};

/// Maps a cursor to a TokenType based on the node's kind.
fn node_kind_to_token_type(kind: &str) -> TokenType {
    match kind {
        "identifier" | "type_identifier" | "field_identifier"
        | "shorthand_property_identifier" | "shorthand_property_identifier_pattern" => {
            TokenType::CodeIdentifier
        }
        "if" | "else" | "for" | "while" | "return" | "let" | "const" | "var"
        | "fn" | "function" | "class" | "struct" | "enum" | "trait" | "impl"
        | "pub" | "use" | "mod" | "import" | "from" | "def" | "lambda"
        | "match" | "loop" | "break" | "continue" | "true" | "false" | "null"
        | "undefined" | "None" | "Some" | "Ok" | "Err" | "as" | "in" | "where"
        | "type" | "let mut" | "if let" | "while let" | "for" | "in" | "match" => {
            TokenType::Keyword
        }
        "string_literal" | "string" | "template_string" | "text" => TokenType::StringLiteral,
        "comment" | "line_comment" | "block_comment" => TokenType::Comment,
        "number_literal" | "number" | "integer_literal" | "float_literal" => TokenType::Number,
        "->" | "=>" | "::" | "+" | "-" | "*" | "/" | "%" | "=" | "==" | "!="
        | "<" | ">" | "<=" | ">=" | "&&" | "||" | "!" | "&" | "|" | "^" | "~"
        | "<<" | ">>" | "+=" | "-=" | "*=" | "/=" | "%=" => TokenType::Operator,
        "(" | ")" | "[" | "]" | "{" | "}" => TokenType::Punctuation,
        "." | "," | ";" | ":" | "#" | "@" | "$" | "?" => TokenType::Punctuation,
        _ => {
            // Check for specific patterns in the kind string
            if kind.contains("comment") {
                TokenType::Comment
            } else if kind.contains("string") || kind.contains("text") {
                TokenType::StringLiteral
            } else if kind.contains("number") || kind.contains("digit") {
                TokenType::Number
            } else {
                TokenType::Other
            }
        }
    }
}

/// A single language grammar entry.
#[derive(Clone)]
pub struct LanguageGrammar {
    /// Display name.
    pub name: &'static str,
    /// File extensions (lowercase, no dot).
    pub extensions: &'static [&'static str],
    /// Shebang patterns to detect.
    pub shebangs: &'static [&'static str],
    /// The tree-sitter language reference.
    pub language: fn() -> tree_sitter::Language,
}

// Language definitions. Each entry must have its `tree-sitter-<lang>` crate
// added to Cargo.toml under `[dependencies]`. If a grammar isn't available
// as a crate at build time, it can be loaded from a .wasm file at runtime.
fn built_in_grammars() -> Vec<LanguageGrammar> {
    vec![
        LanguageGrammar {
            name: "Rust",
            extensions: &["rs"],
            shebangs: &[],
            language: || tree_sitter_rust::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "Go",
            extensions: &["go"],
            shebangs: &[],
            language: || tree_sitter_go::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "Python",
            extensions: &["py", "pyw", "pyx"],
            shebangs: &["python", "python3"],
            language: || tree_sitter_python::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "JavaScript",
            extensions: &["js", "mjs", "cjs", "jsx"],
            shebangs: &["node"],
            language: || tree_sitter_javascript::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "TypeScript",
            extensions: &["ts", "tsx", "mts", "cts"],
            shebangs: &["deno", "tsx"],
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        },
        LanguageGrammar {
            name: "C",
            extensions: &["c", "h"],
            shebangs: &[],
            language: || tree_sitter_c::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "C++",
            extensions: &["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++"],
            shebangs: &[],
            language: || tree_sitter_cpp::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "Java",
            extensions: &["java"],
            shebangs: &[],
            language: || tree_sitter_java::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "Bash",
            extensions: &["sh", "bash", "zsh"],
            shebangs: &["sh", "bash", "zsh", "dash"],
            language: || tree_sitter_bash::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "Markdown",
            extensions: &["md", "markdown"],
            shebangs: &[],
            language: || tree_sitter_md::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "JSON",
            extensions: &["json", "jsonc"],
            shebangs: &[],
            language: || tree_sitter_json::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "TOML",
            extensions: &["toml"],
            shebangs: &[],
            language: || tree_sitter_toml::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "YAML",
            extensions: &["yaml", "yml"],
            shebangs: &[],
            language: || tree_sitter_yaml::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "HTML",
            extensions: &["html", "htm", "xhtml"],
            shebangs: &[],
            language: || tree_sitter_html::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "CSS",
            extensions: &["css", "scss", "less"],
            shebangs: &[],
            language: || tree_sitter_css::LANGUAGE.into(),
        },
        LanguageGrammar {
            name: "SQL",
            extensions: &["sql"],
            shebangs: &[],
            language: || tree_sitter_sql::LANGUAGE.into(),
        },
    ]
}

/// Tree-sitter backed tokenizer that uses AST-aware parsing.
pub struct TreeSitterTokenizer {
    /// Pre-initialized parsers per language.
    parsers: HashMap<&'static str, Parser>,
    /// Language grammar definitions.
    grammars: Vec<LanguageGrammar>,
    /// Default context (prose) when no language is detected.
    default_context: TokenContext,
}

impl TreeSitterTokenizer {
    /// Create a new TreeSitterTokenizer with all built-in grammars.
    pub fn new() -> SpellcastResult<Self> {
        let grammars = built_in_grammars();
        let mut parsers = HashMap::new();

        for g in &grammars {
            let mut parser = Parser::new();
            let lang = (g.language)();
            parser
                .set_language(&lang)
                .map_err(|e| crate::error::SpellcastError::Tokenizer(format!(
                    "Failed to set language '{}': {e}", g.name
                )))?;
            parsers.insert(g.name, parser);
        }

        Ok(Self {
            parsers,
            grammars,
            default_context: TokenContext::Prose,
        })
    }

    /// Create a new TreeSitterTokenizer with only the specified languages.
    pub fn with_languages(languages: &[&str]) -> SpellcastResult<Self> {
        let all = built_in_grammars();
        let grammars: Vec<LanguageGrammar> = all
            .into_iter()
            .filter(|g| languages.contains(&g.name))
            .collect();

        let mut parsers = HashMap::new();
        for g in &grammars {
            let mut parser = Parser::new();
            let lang = (g.language)();
            parser
                .set_language(&lang)
                .map_err(|e| crate::error::SpellcastError::Tokenizer(format!(
                    "Failed to set language '{}': {e}", g.name
                )))?;
            parsers.insert(g.name, parser);
        }

        Ok(Self {
            parsers,
            grammars,
            default_context: TokenContext::Prose,
        })
    }

    /// Detect language from a filename.
    pub fn detect_language_from_filename(&self, filename: &str) -> Option<&'static str> {
        let lower = filename.to_lowercase();
        for g in &self.grammars {
            for ext in g.extensions {
                if lower.ends_with(&format!(".{ext}")) || lower == *ext {
                    return Some(g.name);
                }
            }
        }
        None
    }

    /// Detect language from the first line (shebang).
    pub fn detect_language_from_shebang(&self, first_line: &str) -> Option<&'static str> {
        let trimmed = first_line.trim();
        for g in &self.grammars {
            for shebang in g.shebangs {
                if trimmed.contains(shebang) {
                    return Some(g.name);
                }
            }
        }
        None
    }

    /// Detect language by sampling the text for code syntax patterns.
    pub fn detect_language_from_patterns(&self, text: &str) -> Option<&'static str> {
        // Try each grammar's parser on a sample of the text
        let sample = if text.len() > 200 { &text[..200] } else { text };
        for g in &self.grammars {
            if let Some(parser) = self.parsers.get(g.name) {
                if let Ok(tree) = parser.parse(sample, None) {
                    let root = tree.root_node();
                    // If the tree has meaningful structure beyond a single error node,
                    // this grammar is likely correct
                    if root.child_count() > 0 && !is_single_error(root) {
                        return Some(g.name);
                    }
                }
            }
        }
        None
    }

    /// Get the parser for a named language.
    fn get_parser(&self, name: &str) -> Option<&Parser> {
        self.parsers.get(name)
    }

    /// Recursively walk a tree-sitter node and produce tokens.
    fn node_to_tokens(&self, node: Node, source: &str, tokens: &mut Vec<Token>) {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let kind = node.kind();

        // Map the node kind to our token type
        let token_type = node_kind_to_token_type(kind);

        // Get the text of this node
        let text = &source[start_byte..end_byte];

        // If this is a leaf node (no children), emit a token
        if node.child_count() == 0 {
            // Skip zero-width nodes
            if !text.is_empty() {
                tokens.push(Token {
                    text: text.to_string(),
                    offset: start_byte,
                    length: end_byte - start_byte,
                    token_type,
                });
            }
        } else {
            // For named nodes, we may want to emit the combined token
            // or recurse into children. We emit the parent if it maps
            // to a useful type, then recurse for detail.
            let is_meaningful = matches!(
                token_type,
                TokenType::Comment
                    | TokenType::StringLiteral
                    | TokenType::Keyword
            );
            if is_meaningful && token_type != TokenType::Other {
                // Emit the whole comment/string/keyword as one token
                tokens.push(Token {
                    text: text.to_string(),
                    offset: start_byte,
                    length: end_byte - start_byte,
                    token_type,
                });
            } else {
                // Recurse into children for more granular tokens
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.node_to_tokens(child, source, tokens);
                }
            }
        }
    }

    /// Parse a code block within a markdown file or other nested context.
    pub fn parse_code_block(&self, code: &str, language: &str) -> SpellcastResult<TokenStream> {
        let context = match language {
            "python" | "rust" | "go" | "javascript" | "typescript" | "c" | "cpp"
            | "java" | "bash" | "sql" | "html" | "css" => TokenContext::Code,
            _ => TokenContext::Prose,
        };

        let mut tokens = Vec::new();

        if let Some(parser) = self.get_parser(language) {
            if let Ok(tree) = parser.parse(code, None) {
                let root = tree.root_node();
                self.node_to_tokens(root, code, &mut tokens);
            }
        }

        // Filter whitespace tokens that tree-sitter might produce
        tokens.retain(|t| t.token_type != TokenType::Whitespace || !t.text.trim().is_empty());

        Ok(TokenStream { tokens, context })
    }
}

/// Check if a tree-sitter node represents a single error (no parse).
fn is_single_error(node: Node) -> bool {
    if node.kind() == "ERROR" && node.child_count() == 0 {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !is_single_error(child) {
            return false;
        }
    }
    node.child_count() > 0
}

impl Tokenizer for TreeSitterTokenizer {
    fn tokenize(&self, text: &str) -> SpellcastResult<TokenStream> {
        // First, try to detect a specific language from shebang or patterns
        let first_line = text.lines().next().unwrap_or("");
        let lang = self
            .detect_language_from_shebang(first_line)
            .or_else(|| self.detect_language_from_patterns(text));

        let context = match lang {
            Some(name) if name != "Markdown" => {
                // For code languages, use the code context
                TokenContext::Code
            }
            _ => TokenContext::Prose,
        };

        let mut tokens = Vec::new();

        if let Some(lang_name) = lang {
            if let Some(parser) = self.get_parser(lang_name) {
                if let Ok(tree) = parser.parse(text, None) {
                    let root = tree.root_node();
                    if !is_single_error(root) {
                        // Handle markdown with embedded code blocks
                        if lang_name == "Markdown" {
                            self.tokenize_markdown(text, &tree, &mut tokens)?;
                        } else {
                            self.node_to_tokens(root, text, &mut tokens);
                        }
                        tokens.retain(|t| t.token_type != TokenType::Whitespace || !t.text.trim().is_empty());
                        return Ok(TokenStream { tokens, context });
                    }
                }
            }
        }

        // Fallback: if no grammar matched or parse failed, use prose heuristic
        fallback_tokenize(text, &mut tokens);
        Ok(TokenStream {
            tokens,
            context: TokenContext::Prose,
        })
    }

    fn tokenize_with_context(&self, text: &str, context: TokenContext) -> SpellcastResult<TokenStream> {
        // When context is explicitly provided, still try the matched grammar
        // but use the given context for the token stream
        let mut stream = self.tokenize(text)?;
        stream.context = context;
        Ok(stream)
    }

    fn detect_context(&self, text: &str) -> TokenContext {
        let first_line = text.lines().next().unwrap_or("");
        let lang = self
            .detect_language_from_shebang(first_line)
            .or_else(|| self.detect_language_from_patterns(text));

        match lang {
            Some(name) if name != "Markdown" => TokenContext::Code,
            _ => TokenContext::Prose,
        }
    }
}

/// Tokenize markdown, handling embedded code blocks with their own grammar.
impl TreeSitterTokenizer {
    fn tokenize_markdown(
        &self,
        text: &str,
        tree: &Tree,
        tokens: &mut Vec<Token>,
    ) -> SpellcastResult<()> {
        let root = tree.root_node();
        let mut cursor = root.walk();

        for node in root.children(&mut cursor) {
            let kind = node.kind();

            if kind == "fenced_code_block" {
                // Extract the language from the info string
                let info_string = self.get_fence_info(node, text);
                let code_text = &text[node.start_byte..node.end_byte];

                // Parse the code block with the appropriate grammar
                if let Some(lang) = info_string {
                    if let Ok(sub_stream) = self.parse_code_block(code_text, &lang) {
                        tokens.extend(sub_stream.tokens);
                        continue;
                    }
                }

                // Fallback: treat as prose
                fallback_tokenize(code_text, tokens);
            } else {
                // Recurse into other markdown nodes
                let mut child_cursor = node.walk();
                for child in node.children(&mut child_cursor) {
                    let child_text = &text[child.start_byte..child.end_byte];
                    if !child_text.trim().is_empty() {
                        tokens.push(Token {
                            text: child_text.to_string(),
                            offset: child.start_byte,
                            length: child.end_byte - child.start_byte,
                            token_type: TokenType::Word,
                        });
                    }
                }

                // If no children, emit the node text as a prose token
                if node.child_count() == 0 {
                    let node_text = &text[node.start_byte..node.end_byte];
                    if !node_text.trim().is_empty() {
                        tokens.push(Token {
                            text: node_text.to_string(),
                            offset: node.start_byte,
                            length: node.end_byte - node.start_byte,
                            token_type: TokenType::Word,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Fenced code block info string cursor for markdown tokenization.
fn _get_fence_info(node: Node, text: &str) -> Option<&str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "info_string" {
                let info = &text[child.start_byte..child.end_byte];
                return Some(info.trim());
            }
        }
        None
    }
}

/// Simple fallback tokenizer for when no grammar is available.
/// Splits text into words, punctuation, whitespace.
fn fallback_tokenize(text: &str, tokens: &mut Vec<Token>) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();
    let mut pos = 0;

    while pos < len {
        let (byte_pos, ch) = chars[pos];

        if ch.is_ascii_whitespace() {
            let start = pos;
            while pos < len && chars[pos].1.is_ascii_whitespace() {
                pos += 1;
            }
            let end_byte = if pos < len { chars[pos].0 } else { text.len() };
            tokens.push(Token {
                text: text[chars[start].0..end_byte].to_string(),
                offset: chars[start].0,
                length: end_byte - chars[start].0,
                token_type: TokenType::Whitespace,
            });
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let start = pos;
            while pos < len {
                let c = chars[pos].1;
                if c.is_ascii_alphanumeric() || c == '_' || c == '\'' {
                    pos += 1;
                } else {
                    break;
                }
            }
            let end_byte = if pos < len { chars[pos].0 } else { text.len() };
            let word = &text[chars[start].0..end_byte];
            let token_type = if word.contains('_') || word.chars().any(|c| c.is_ascii_uppercase()) {
                TokenType::CodeIdentifier
            } else {
                TokenType::Word
            };
            tokens.push(Token {
                text: word.to_string(),
                offset: chars[start].0,
                length: end_byte - chars[start].0,
                token_type,
            });
        } else {
            tokens.push(Token {
                text: ch.to_string(),
                offset: byte_pos,
                length: ch.len_utf8(),
                token_type: if ch.is_ascii_digit() || ch == '.' {
                    TokenType::Number
                } else {
                    TokenType::Punctuation
                },
            });
            pos += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> TreeSitterTokenizer {
        TreeSitterTokenizer::new().expect("Failed to create tokenizer")
    }

    #[test]
    fn test_detect_filename_rust() {
        let t = setup();
        assert_eq!(
            t.detect_language_from_filename("main.rs"),
            Some("Rust")
        );
    }

    #[test]
    fn test_detect_filename_python() {
        let t = setup();
        assert_eq!(
            t.detect_language_from_filename("script.py"),
            Some("Python")
        );
    }

    #[test]
    fn test_detect_filename_unknown() {
        let t = setup();
        assert_eq!(t.detect_language_from_filename("README.txt"), None);
    }

    #[test]
    fn test_detect_shebang_python() {
        let t = setup();
        assert_eq!(
            t.detect_language_from_shebang("#!/usr/bin/env python3"),
            Some("Python")
        );
    }

    #[test]
    fn test_detect_shebang_bash() {
        let t = setup();
        assert_eq!(
            t.detect_language_from_shebang("#!/bin/bash"),
            Some("Bash")
        );
    }

    #[test]
    fn test_tokenize_rust_code() {
        let t = setup();
        let source = "fn main() { let x = 42; }";
        let stream = t.tokenize(source).unwrap();
        assert!(!stream.tokens.is_empty());
        assert_eq!(stream.context, TokenContext::Code);

        // Should have identifier tokens
        let idents: Vec<_> = stream
            .tokens
            .iter()
            .filter(|tok| tok.token_type == TokenType::CodeIdentifier)
            .collect();
        assert!(!idents.is_empty(), "rust code should produce identifiers");
    }

    #[test]
    fn test_tokenize_python_code() {
        let t = setup();
        let source = "def hello(name):\n    print(f\"Hello, {name}\")";
        let stream = t.tokenize(source).unwrap();
        assert!(!stream.tokens.is_empty());

        let idents: Vec<_> = stream
            .tokens
            .iter()
            .filter(|tok| tok.token_type == TokenType::CodeIdentifier)
            .collect();
        assert!(!idents.is_empty(), "python code should produce identifiers");
    }

    #[test]
    fn test_tokenize_prose_fallback() {
        let t = setup();
        let source = "Hello world, this is a test.";
        let stream = t.tokenize(source).unwrap();
        assert_eq!(stream.context, TokenContext::Prose);

        let words: Vec<_> = stream
            .tokens
            .iter()
            .filter(|tok| tok.token_type == TokenType::Word)
            .collect();
        assert_eq!(words.len(), 6);
    }

    #[test]
    fn test_tokenize_js_arrow_function() {
        let t = setup();
        let source = "const add = (a, b) => a + b;";
        let stream = t.tokenize(source).unwrap();
        assert_eq!(stream.context, TokenContext::Code);

        let operators: Vec<_> = stream
            .tokens
            .iter()
            .filter(|tok| tok.token_type == TokenType::Operator)
            .collect();
        // Should have => and + operators
        let op_texts: Vec<&str> = operators.iter().map(|t| t.text.as_str()).collect();
        assert!(op_texts.contains(&"=>") || op_texts.contains(&"+"), "JS arrow should produce operators");
    }

    #[test]
    fn test_node_kind_mapping() {
        assert_eq!(node_kind_to_token_type("identifier"), TokenType::CodeIdentifier);
        assert_eq!(node_kind_to_token_type("fn"), TokenType::Keyword);
        assert_eq!(node_kind_to_token_type("string_literal"), TokenType::StringLiteral);
        assert_eq!(node_kind_to_token_type("comment"), TokenType::Comment);
        assert_eq!(node_kind_to_token_type("->"), TokenType::Operator);
        assert_eq!(node_kind_to_token_type("("), TokenType::Punctuation);
        assert_eq!(node_kind_to_token_type("number_literal"), TokenType::Number);
        assert_eq!(node_kind_to_token_type("unknown_thing"), TokenType::Other);
    }

    #[test]
    fn test_is_single_error() {
        // Create a minimal tree to check
        let mut parser = Parser::new();
        let rust_lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&rust_lang).unwrap();
        // Invalid Rust should produce an error tree
        let tree = parser.parse("@@@invalid@@@", None).unwrap();
        let root = tree.root_node();
        // Root should have error children
        assert!(root.has_error());
    }

    #[test]
    fn test_grammar_list_not_empty() {
        let g = built_in_grammars();
        assert!(g.len() >= 13, "should have at least 13 languages");
    }
}