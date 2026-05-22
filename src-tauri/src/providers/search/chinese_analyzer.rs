//! Chinese retrieval uses character bigrams (2-gram) over the combined index text.
//! This avoids relying on Tantivy's default English tokenizer for CJK content.

use tantivy::tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer};

pub fn cjk_bigram_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(NgramTokenizer::new(2, 2, false).expect("valid ngram range"))
        .filter(LowerCaser)
        .build()
}

#[cfg(test)]
mod tests {
    use super::cjk_bigram_analyzer;
    use tantivy::tokenizer::TokenStream;

    #[test]
    fn analyzer_emits_bigrams_for_chinese_text() {
        let mut analyzer = cjk_bigram_analyzer();
        let mut stream = analyzer.token_stream("合同审查要点");
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().text.clone());
        }
        assert!(tokens.contains(&"合同".to_string()));
        assert!(tokens.contains(&"审查".to_string()));
    }
}
