// SPDX-License-Identifier: Apache-2.0

//! Token-aware text tokenization.
//!
//! Tokens are language- and context-aware units that can be code identifiers,
//! punctuation, prose words, or operators. The tokenizer determines token
//! boundaries based on the detected context (prose vs code).

use regex::Regex;

use crate::error::VoxKeyResult;

/// The detected context type for tokenization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenContext {
    /// Natural language prose (words, punctuation, contractions)
    Prose,
    /// Programming code (identifiers, operators, keywords)
    Code,
}

/// A single token with metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token text
    pub text: String,
    /// Byte offset in the source string
    pub offset: usize,
    /// Byte length of the token
    pub length: usize,
    /// Token type classification
    pub token_type: TokenType,
}

/// Classification of a token's type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    /// A natural language word (e.g., "hello", "don't")
    Word,
    /// A code identifier (e.g., "fooBar", "foo_bar")
    CodeIdentifier,
    /// A punctuation character (e.g., ".", ",", "!")
    Punctuation,
    /// An operator (e.g., "->", "=>", "::")
    Operator,
    /// Whitespace
    Whitespace,
    /// A number literal (e.g., "42", "3.14")
    Number,
    /// A string literal delimiter or content
    StringLiteral,
    /// Any other token
    Other,
}

/// A sequence of tokens produced by the tokenizer.
#[derive(Debug, Clone)]
pub struct TokenStream {
    /// The tokens in order
    pub tokens: Vec<Token>,
    /// The detected context
    pub context: TokenContext,
}

impl TokenStream {
    /// Create an empty token stream.
    pub fn new(context: TokenContext) -> Self {
        Self {
            tokens: Vec::new(),
            context,
        }
    }

    /// Get the number of tokens.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if the stream is empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Get a token by index.
    pub fn get(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    /// Remove a token by index.
    pub fn remove(&mut self, index: usize) -> Option<Token> {
        if index < self.tokens.len() {
            Some(self.tokens.remove(index))
        } else {
            None
        }
    }

    /// Insert a token at a given index.
    pub fn insert(&mut self, index: usize, token: Token) {
        if index <= self.tokens.len() {
            self.tokens.insert(index, token);
        }
    }

    /// Replace a token at a given index.
    pub fn replace(&mut self, index: usize, token: Token) -> Option<Token> {
        if index < self.tokens.len() {
            Some(std::mem::replace(&mut self.tokens[index], token))
        } else {
            None
        }
    }
}

/// Tokenizer trait.
pub trait Tokenizer: Send {
    /// Tokenize the given text, detecting context automatically.
    fn tokenize(&self, text: &str) -> VoxKeyResult<TokenStream>;

    /// Tokenize with an explicit context hint.
    fn tokenize_with_context(&self, text: &str, context: TokenContext) -> VoxKeyResult<TokenStream>;

    /// Detect the context of the given text.
    fn detect_context(&self, text: &str) -> TokenContext;
}

/// Heuristic tokenizer — uses regex-based pattern matching.
///
/// Detects context by sampling the text for common code patterns
/// (camelCase, snake_case, operators, keywords).
pub struct HeuristicTokenizer {
    /// Regex for prose words (letters, contractions, hyphens)
    word_re: Regex,
    /// Regex for code identifiers (camelCase, snake_case)
    code_id_re: Regex,
    /// Regex for operators
    operator_re: Regex,
    /// Regex for numbers
    number_re: Regex,
}

impl HeuristicTokenizer {
    /// Create a new HeuristicTokenizer.
    pub fn new() -> Self {
        Self {
            word_re: Regex::new(r"[a-zA-Z]+(?:[''][a-zA-Z]+)?").unwrap(),
            code_id_re: Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*(?:[A-Z][a-z]+)*").unwrap(),
            operator_re: Regex::new(r"->|=>|::|\+\+|--|==|!=|<=|>=|&&|\|\||<<|>>|[+\-*/%=<>!&|^~]").unwrap(),
            number_re: Regex::new(r"\d+(?:\.\d+)?").unwrap(),
        }
    }

    /// Check if text contains code-like patterns.
    fn has_code_patterns(&self, text: &str) -> bool {
        let code_indicators = [
            "->", "=>", "::", "fn ", "let ", "if ", "else", "return",
            "pub ", "struct", "impl", "match", "while", "for ", "loop",
        ];
        code_indicators.iter().any(|&kw| text.contains(kw))
    }

    /// Count camelCase and snake_case patterns.
    fn count_code_identifiers(&self, text: &str) -> usize {
        let camel_case = Regex::new(r"[a-z]+[A-Z][a-z]+").unwrap();
        let snake_case = Regex::new(r"[a-z]+_[a-z]+").unwrap();
        camel_case.find_iter(text).count() + snake_case.find_iter(text).count()
    }
}

impl Default for HeuristicTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer for HeuristicTokenizer {
    fn tokenize(&self, text: &str) -> VoxKeyResult<TokenStream> {
        let context = self.detect_context(text);
        self.tokenize_with_context(text, context)
    }

    fn tokenize_with_context(&self, text: &str, _context: TokenContext) -> VoxKeyResult<TokenStream> {
        let mut tokens = Vec::new();
        let mut pos = 0;
        let bytes = text.as_bytes();
        let len = bytes.len();

        while pos < len {
            // Skip whitespace first
            if bytes[pos].is_ascii_whitespace() {
                let start = pos;
                while pos < len && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                tokens.push(Token {
                    text: text[start..pos].to_string(),
                    offset: start,
                    length: pos - start,
                    token_type: TokenType::Whitespace,
                });
                continue;
            }

            // Check for operator
            if pos + 1 < len {
                let two_char = &text[pos..pos + 2];
                if self.operator_re.is_match(two_char) {
                    tokens.push(Token {
                        text: two_char.to_string(),
                        offset: pos,
                        length: 2,
                        token_type: TokenType::Operator,
                    });
                    pos += 2;
                    continue;
                }
            }

            // Single char operator
            if self.operator_re.is_match(&text[pos..pos + 1]) {
                tokens.push(Token {
                    text: text[pos..pos + 1].to_string(),
                    offset: pos,
                    length: 1,
                    token_type: TokenType::Operator,
                });
                pos += 1;
                continue;
            }

            // Number literal
            if bytes[pos].is_ascii_digit() {
                let start = pos;
                while pos < len && (bytes[pos].is_ascii_digit() || bytes[pos] == b'.') {
                    pos += 1;
                }
                tokens.push(Token {
                    text: text[start..pos].to_string(),
                    offset: start,
                    length: pos - start,
                    token_type: TokenType::Number,
                });
                continue;
            }

            // Word (prose or code identifier)
            if bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' {
                let start = pos;
                while pos < len && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_' || bytes[pos] == b'\'') {
                    pos += 1;
                }
                let word = &text[start..pos];

                // Determine if it's a code identifier or prose word
                let token_type = if word.contains('_') || word.chars().any(|c| c.is_ascii_uppercase()) {
                    TokenType::CodeIdentifier
                } else {
                    TokenType::Word
                };

                tokens.push(Token {
                    text: word.to_string(),
                    offset: start,
                    length: pos - start,
                    token_type,
                });
                continue;
            }

            // Punctuation and other characters
            let start = pos;
            pos += 1;
            tokens.push(Token {
                text: text[start..pos].to_string(),
                offset: start,
                length: 1,
                token_type: TokenType::Punctuation,
            });
        }

        Ok(TokenStream {
            tokens,
            context: _context,
        })
    }

    fn detect_context(&self, text: &str) -> TokenContext {
        if text.is_empty() {
            return TokenContext::Prose;
        }

        if self.has_code_patterns(text) {
            return TokenContext::Code;
        }

        if self.count_code_identifiers(text) >= 2 {
            return TokenContext::Code;
        }

        TokenContext::Prose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_tokenizer() -> HeuristicTokenizer {
        HeuristicTokenizer::new()
    }

    #[test]
    fn test_tokenize_prose() {
        let t = setup_tokenizer();
        let stream = t.tokenize("hello world").unwrap();
        assert_eq!(stream.len(), 3); // "hello", " ", "world"
        assert_eq!(stream.tokens[0].text, "hello");
        assert_eq!(stream.tokens[0].token_type, TokenType::Word);
        assert_eq!(stream.tokens[2].text, "world");
    }

    #[test]
    fn test_tokenize_code_identifier() {
        let t = setup_tokenizer();
        let stream = t.tokenize_with_context("fooBar", TokenContext::Code).unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream.tokens[0].token_type, TokenType::CodeIdentifier);
    }

    #[test]
    fn test_tokenize_operator() {
        let t = setup_tokenizer();
        let stream = t.tokenize("a -> b").unwrap();
        assert_eq!(stream.tokens[2].text, "->");
        assert_eq!(stream.tokens[2].token_type, TokenType::Operator);
    }

    #[test]
    fn test_tokenize_numbers() {
        let t = setup_tokenizer();
        let stream = t.tokenize("42 3.14").unwrap();
        assert_eq!(stream.tokens[0].text, "42");
        assert_eq!(stream.tokens[0].token_type, TokenType::Number);
        assert_eq!(stream.tokens[2].text, "3.14");
        assert_eq!(stream.tokens[2].token_type, TokenType::Number);
    }

    #[test]
    fn test_detect_code_context() {
        let t = setup_tokenizer();
        let ctx = t.detect_context("fn hello() { let x = 42; }");
        assert_eq!(ctx, TokenContext::Code);
    }

    #[test]
    fn test_detect_prose_context() {
        let t = setup_tokenizer();
        let ctx = t.detect_context("hello world, how are you?");
        assert_eq!(ctx, TokenContext::Prose);
    }

    #[test]
    fn test_detect_code_by_identifiers() {
        let t = setup_tokenizer();
        let ctx = t.detect_context("fooBar bazQux");
        assert_eq!(ctx, TokenContext::Code);
    }

    #[test]
    fn test_token_stream_operations() {
        let mut stream = TokenStream::new(TokenContext::Prose);
        stream.tokens.push(Token {
            text: "hello".to_string(),
            offset: 0,
            length: 5,
            token_type: TokenType::Word,
        });
        stream.tokens.push(Token {
            text: "world".to_string(),
            offset: 6,
            length: 5,
            token_type: TokenType::Word,
        });

        assert_eq!(stream.len(), 2);
        assert_eq!(stream.get(0).unwrap().text, "hello");

        let removed = stream.remove(0).unwrap();
        assert_eq!(removed.text, "hello");
        assert_eq!(stream.len(), 1);

        stream.insert(0, removed);
        assert_eq!(stream.len(), 2);
    }

    #[test]
    fn test_empty_stream() {
        let stream = TokenStream::new(TokenContext::Prose);
        assert!(stream.is_empty());
        assert_eq!(stream.len(), 0);
    }

    #[test]
    fn test_replace_token() {
        let mut stream = TokenStream::new(TokenContext::Prose);
        stream.tokens.push(Token {
            text: "hello".to_string(),
            offset: 0,
            length: 5,
            token_type: TokenType::Word,
        });

        let replacement = Token {
            text: "hi".to_string(),
            offset: 0,
            length: 2,
            token_type: TokenType::Word,
        };

        let old = stream.replace(0, replacement).unwrap();
        assert_eq!(old.text, "hello");
        assert_eq!(stream.tokens[0].text, "hi");
    }

    #[test]
    fn test_tokenize_contraction() {
        let t = setup_tokenizer();
        let stream = t.tokenize("don't").unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream.tokens[0].text, "don't");
    }

    #[test]
    fn test_tokenize_snake_case() {
        let t = setup_tokenizer();
        let stream = t.tokenize_with_context("my_variable_name", TokenContext::Code).unwrap();
        assert_eq!(stream.len(), 1);
        assert_eq!(stream.tokens[0].token_type, TokenType::CodeIdentifier);
    }
}