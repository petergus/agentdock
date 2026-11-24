//! Local `cl100k_base` token estimation powered by `tiktoken-rs`.

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

use crate::models::SKILL_UPFRONT_OVERHEAD_TOKENS;

static BPE: OnceLock<Option<CoreBPE>> = OnceLock::new();

fn encoder() -> Option<&'static CoreBPE> {
    BPE.get_or_init(|| tiktoken_rs::cl100k_base().ok()).as_ref()
}

/// Counts exact `cl100k_base` tokens for `text`.
///
/// Uses `encode_ordinary` so literal special-token strings inside markdown files
/// cannot panic, and falls back gracefully to a char heuristic only if BPE fails
/// to initialize.
#[must_use]
pub fn count_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    match encoder() {
        Some(bpe) => bpe.encode_ordinary(text).len(),
        None => text.chars().count().div_ceil(4),
    }
}

/// Computes upfront prompt overhead for a skill entry (`name + description + wrapper`).
#[must_use]
pub fn skill_upfront_tokens(name: &str, description: &str) -> usize {
    count_tokens(name) + count_tokens(description) + SKILL_UPFRONT_OVERHEAD_TOKENS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_empty_and_normal_text() {
        assert_eq!(count_tokens(""), 0);
        let tokens = count_tokens("Hello world, this is AgentDock.");
        assert!((6..=10).contains(&tokens), "unexpected count: {tokens}");
    }

    #[test]
    fn handles_special_token_literals_safely() {
        let count = count_tokens("<|endoftext|> <|fim_prefix|> test");
        assert!(count > 0);
    }

    #[test]
    fn skill_upfront_includes_structural_overhead() {
        let bare = count_tokens("apple-design") + count_tokens("Reviews macOS UI.");
        let upfront = skill_upfront_tokens("apple-design", "Reviews macOS UI.");
        assert_eq!(upfront, bare + SKILL_UPFRONT_OVERHEAD_TOKENS);
    }
}
