//! Helpers shared by `corvene-core`'s ports of GitHub Desktop's tokenizer
//! tests (`unit/text-token-parser-test.ts`,
//! `unit/wrap-rich-text-commit-message-test.ts`): GitHub Desktop's
//! `TokenResult` accessors over Corvene's `text_tokens::Token`.

use corvene_core::text_tokens::Token;

/// GitHub Desktop's `TokenType`.
#[derive(Debug, PartialEq, Eq)]
pub enum TokenType {
    Text,
    Emoji,
    Link,
}

/// GitHub Desktop's `token.kind`.
pub fn kind(token: &Token) -> TokenType {
    match token {
        Token::Text(_) => TokenType::Text,
        Token::Emoji { .. } => TokenType::Emoji,
        Token::Link { .. } => TokenType::Link,
    }
}

/// GitHub Desktop's `token.text`.
pub fn text(token: &Token) -> &str {
    match token {
        Token::Text(text) | Token::Emoji { text, .. } | Token::Link { text, .. } => text,
    }
}

/// GitHub Desktop's `(token as HyperlinkMatch).url`.
pub fn url(token: &Token) -> &str {
    match token {
        Token::Link { url, .. } => url,
        other => panic!("not a HyperlinkMatch: {other:?}"),
    }
}
