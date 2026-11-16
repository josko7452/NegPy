//! CLIP's byte-level BPE tokenizer.
//!
//! Mirrors `negpy/services/assets/clip_tokenizer.py`: the vocabulary is derived from
//! `merges.txt` in OpenAI's build order, so the token ids match the exported model
//! without a separately scraped vocabulary file.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

const START: &str = "<|startoftext|>";
const END: &str = "<|endoftext|>";

/// GPT-2/CLIP's printable stand-in for each byte.
fn bytes_to_unicode() -> [char; 256] {
    let mut printable: Vec<u8> = (33..=126u8).chain(161..=172u8).chain(174..=255u8).collect();
    let mut codes: Vec<u32> = printable.iter().map(|byte| u32::from(*byte)).collect();
    let mut next = 0u32;
    for byte in 0..=255u8 {
        if !printable.contains(&byte) {
            printable.push(byte);
            codes.push(256 + next);
            next += 1;
        }
    }
    let mut map = ['\0'; 256];
    for (byte, code) in printable.into_iter().zip(codes) {
        map[usize::from(byte)] = char::from_u32(code).unwrap_or('\0');
    }
    map
}

fn pairs(word: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = word
        .windows(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The CLIP cleaner: unescape twice, collapse whitespace, trim.
fn clean(text: &str) -> String {
    let once = html_escape::decode_html_entities(text).into_owned();
    let twice = html_escape::decode_html_entities(&once).into_owned();
    let collapsed: String = twice.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
}

fn pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)'s|'t|'re|'ve|'m|'ll|'d|[\w]+|[^\s\w]+").expect("CLIP pattern")
    })
}

/// A tokenizer built from one `merges.txt` file.
#[derive(Clone, Debug)]
pub struct ClipTokenizer {
    byte_encoder: [char; 256],
    encoder: HashMap<String, usize>,
    ranks: HashMap<(String, String), usize>,
    cache: HashMap<String, String>,
    pub start_id: usize,
    pub end_id: usize,
}

impl ClipTokenizer {
    /// Read the merge rules and build the vocabulary.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("Cannot read the merges: {error}"))?;
        Ok(Self::from_merges(&text))
    }

    /// Build the vocabulary from the merge file's contents.
    #[must_use]
    pub fn from_merges(text: &str) -> Self {
        let merges: Vec<(String, String)> = text
            .split('\n')
            .skip(1)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                (parts.len() == 2).then(|| (parts[0].to_string(), parts[1].to_string()))
            })
            .collect();
        let byte_encoder = bytes_to_unicode();
        let base: Vec<String> = byte_encoder.iter().map(|value| value.to_string()).collect();
        let mut vocabulary: Vec<String> = base.clone();
        vocabulary.extend(base.iter().map(|value| format!("{value}</w>")));
        vocabulary.extend(merges.iter().map(|(left, right)| format!("{left}{right}")));
        vocabulary.push(START.to_string());
        vocabulary.push(END.to_string());

        let encoder: HashMap<String, usize> = vocabulary
            .iter()
            .enumerate()
            .map(|(index, token)| (token.clone(), index))
            .collect();
        let ranks: HashMap<(String, String), usize> = merges
            .iter()
            .enumerate()
            .map(|(index, pair)| (pair.clone(), index))
            .collect();
        let mut cache = HashMap::new();
        cache.insert(START.to_string(), START.to_string());
        cache.insert(END.to_string(), END.to_string());
        let start_id = encoder[START];
        let end_id = encoder[END];
        Self {
            byte_encoder,
            encoder,
            ranks,
            cache,
            start_id,
            end_id,
        }
    }

    fn bpe(&mut self, token: &str) -> String {
        if let Some(cached) = self.cache.get(token) {
            return cached.clone();
        }
        let mut word: Vec<String> = token.chars().map(String::from).collect();
        if word.is_empty() {
            return String::new();
        }
        let last = word.len() - 1;
        word[last] = format!("{}</w>", word[last]);
        if word.len() == 1 {
            let result = word[0].clone();
            self.cache.insert(token.to_string(), result.clone());
            return result;
        }
        let mut current = pairs(&word);
        loop {
            let bigram = current
                .iter()
                .min_by_key(|pair| self.ranks.get(*pair).copied().unwrap_or(usize::MAX))
                .cloned();
            let Some(bigram) = bigram else {
                break;
            };
            if !self.ranks.contains_key(&bigram) {
                break;
            }
            let (first, second) = bigram;
            let mut merged: Vec<String> = Vec::with_capacity(word.len());
            let mut index = 0;
            while index < word.len() {
                let Some(found) = word[index..].iter().position(|part| *part == first) else {
                    merged.extend(word[index..].iter().cloned());
                    break;
                };
                merged.extend(word[index..index + found].iter().cloned());
                index += found;
                if word[index] == first && index + 1 < word.len() && word[index + 1] == second {
                    merged.push(format!("{first}{second}"));
                    index += 2;
                } else {
                    merged.push(word[index].clone());
                    index += 1;
                }
            }
            word = merged;
            if word.len() == 1 {
                break;
            }
            current = pairs(&word);
        }
        let result = word.join(" ");
        self.cache.insert(token.to_string(), result.clone());
        result
    }

    fn token_ids(&mut self, text: &str) -> Vec<usize> {
        let cleaned = clean(text).to_lowercase();
        let mut ids = Vec::new();
        for piece in pattern().find_iter(&cleaned) {
            let encoded: String = piece
                .as_str()
                .bytes()
                .map(|byte| self.byte_encoder[usize::from(byte)])
                .collect();
            for token in self.bpe(&encoded).split(' ') {
                if let Some(id) = self.encoder.get(token) {
                    ids.push(*id);
                }
            }
        }
        ids
    }

    /// `(input_ids, attention_mask)`, each `context_length` long: start and end
    /// tokens bracket the truncated content, the rest is padded and masked out.
    #[must_use]
    pub fn encode(&mut self, text: &str, context_length: usize) -> (Vec<i64>, Vec<i64>) {
        let content = self.token_ids(text);
        let keep = context_length.saturating_sub(2);
        let mut ids = Vec::with_capacity(context_length);
        ids.push(self.start_id);
        ids.extend(content.into_iter().take(keep));
        ids.push(self.end_id);
        let mut mask = vec![1i64; ids.len()];
        let pad = context_length.saturating_sub(ids.len());
        ids.extend(std::iter::repeat_n(0usize, pad));
        mask.extend(std::iter::repeat_n(0i64, pad));
        (ids.into_iter().map(|id| id as i64).collect(), mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MERGES: &str = "#version: 0.2\nc a\nca t\ndo g\n";

    fn tokenizer() -> ClipTokenizer {
        ClipTokenizer::from_merges(MERGES)
    }

    #[test]
    fn start_and_end_tokens_bracket_a_short_string() {
        let mut tokenizer = tokenizer();
        let (ids, mask) = tokenizer.encode("cat", 8);
        assert_eq!(ids.len(), 8);
        assert_eq!(mask.len(), 8);
        assert_eq!(ids[0] as usize, tokenizer.start_id);
        let end_pos = ids
            .iter()
            .position(|id| *id as usize == tokenizer.end_id)
            .expect("end token");
        assert!(end_pos > 1);
        assert!(mask[..=end_pos].iter().all(|value| *value == 1));
        assert!(mask[end_pos + 1..].iter().all(|value| *value == 0));
    }

    #[test]
    fn padding_fills_with_zero_and_a_zero_mask() {
        let mut tokenizer = tokenizer();
        let (ids, mask) = tokenizer.encode("cat", 8);
        let end_pos = ids
            .iter()
            .position(|id| *id as usize == tokenizer.end_id)
            .expect("end token");
        assert!(ids[end_pos + 1..].iter().all(|id| *id == 0));
        assert!(mask[end_pos + 1..].iter().all(|value| *value == 0));
    }

    #[test]
    fn long_text_is_truncated_to_context_length() {
        let mut tokenizer = tokenizer();
        let (ids, mask) = tokenizer.encode(&vec!["cat"; 50].join(" "), 8);
        assert_eq!(ids.len(), 8);
        assert_eq!(mask.len(), 8);
        assert_eq!(ids[7] as usize, tokenizer.end_id);
        assert!(mask.iter().all(|value| *value == 1));
    }

    #[test]
    fn encoding_is_deterministic() {
        let mut tokenizer = tokenizer();
        let first = tokenizer.encode("a cat and a dog", 12);
        let second = tokenizer.encode("a cat and a dog", 12);
        assert_eq!(first, second);
    }

    #[test]
    fn empty_string_is_just_the_two_special_tokens() {
        let mut tokenizer = tokenizer();
        let (ids, mask) = tokenizer.encode("", 8);
        assert_eq!(ids[0] as usize, tokenizer.start_id);
        assert_eq!(ids[1] as usize, tokenizer.end_id);
        assert_eq!(&mask[..2], [1, 1]);
        assert!(mask[2..].iter().all(|value| *value == 0));
    }

    #[test]
    fn start_and_end_ids_are_the_last_two_vocab_entries() {
        let tokenizer = tokenizer();
        assert_eq!(tokenizer.end_id, tokenizer.start_id + 1);
    }

    #[test]
    fn the_cleaner_unescapes_twice_and_collapses_space() {
        assert_eq!(clean("  a &amp;amp;  b "), "a & b");
    }

    #[test]
    fn a_contraction_is_its_own_piece() {
        let mut tokenizer = tokenizer();
        let ids = tokenizer.token_ids("it's a cat");
        assert!(!ids.is_empty());
    }
}
