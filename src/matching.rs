use frizbee::{CaseMatching, Config as MatcherConfig, Matcher, Pattern, SortStrategy};

use crate::index::{Kind, Source, Spelling};
use crate::{ALIAS, Config, IDENTIFIER, KEYWORD, LOCALIZED_NAME, PRIMARY_NAME};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Rank {
    exact_name: bool,
    pub score: i32,
}

struct Scores {
    text: i32,
    name: i32,
    #[cfg(feature = "pinyin")]
    initials: i32,
    exact: bool,
}

impl Scores {
    fn rank(&self, source: &Source) -> Rank {
        let score = match source.kind {
            Kind::Literal if source.role <= ALIAS => self.name,
            Kind::Literal => self.text,
            #[cfg(feature = "pinyin")]
            Kind::Pinyin => self.text - 40,
            #[cfg(feature = "pinyin")]
            Kind::Initials => self.initials - 80,
        };
        let field_bonus = match source.role {
            PRIMARY_NAME => 160,
            LOCALIZED_NAME => 120,
            ALIAS => 80,
            KEYWORD => 0,
            IDENTIFIER => -40,
            _ => unreachable!(),
        };

        Rank {
            exact_name: self.exact && source.kind == Kind::Literal && source.role <= ALIAS,
            score: score + field_bonus,
        }
    }
}

pub fn matches<'a>(
    query: &'a str,
    spellings: &'a [Spelling],
    config: &Config,
) -> impl Iterator<Item = (&'a Source, Rank)> {
    let mut settings = MatcherConfig {
        max_typos: Some(config.max_typos),
        casing: CaseMatching::Ignore,
        sort: SortStrategy::IndexAsc,
        ..MatcherConfig::default()
    };
    settings.scoring.matching_case_bonus = 0;
    settings.scoring.exact_match_bonus = 0;

    // Construct literal patterns: punctuation is input, not Frizbee query
    // syntax.
    let patterns: Vec<Pattern> = query.split_whitespace().map(Pattern::from).collect();
    let query_chars: usize = patterns
        .iter()
        .map(|pattern| pattern.needle.chars().count())
        .sum();
    let completion_score = query_chars * usize::from(settings.scoring.match_score)
        + patterns.len() * usize::from(settings.scoring.prefix_bonus);
    let mut matcher = Matcher::from_patterns(&patterns, &settings);
    let hits = matcher.match_list(spellings);
    let compact = query.replace(' ', "");

    hits.into_iter().flat_map(move |hit| {
        let spelling = &spellings[hit.index as usize];
        let text = spelling.text.as_ref();
        let score_text = |text: &str, score: u16| {
            // A perfect completion is worth 1000 before coverage bonuses.
            let mut quality = (usize::from(score) * 1_000 / completion_score) as i32;
            let contiguous = patterns
                .iter()
                .all(|pattern| text.contains(&pattern.needle));
            let mut initials = text
                .split_whitespace()
                .map(|word| word.chars().next().unwrap());
            let abbreviation = compact
                .chars()
                .all(|ch| initials.by_ref().any(|initial| initial == ch));

            // Keep substrings and word initials strong away from the start.
            if contiguous || abbreviation {
                quality = quality.max(950) + 150;
            }

            let chars = text.chars().filter(|&ch| ch != ' ').count();
            let coverage = (300 * query_chars.min(chars) / chars) as i32;

            (quality, coverage)
        };
        let (quality, coverage) = score_text(text, hit.score);
        let text_score = quality + coverage;
        let mut name_score = text_score;

        // Score name components such as Writer in LibreOffice Writer, using
        // the same matcher. Sentence fragments in descriptions get no bonus.
        if text.contains(' ')
            && spelling
                .sources
                .iter()
                .any(|source| source.role <= ALIAS && source.kind == Kind::Literal)
        {
            for word in text.split_whitespace() {
                let Some(word_match) = matcher.match_one(word, 0) else {
                    continue;
                };
                let (quality, coverage) = score_text(word, word_match.score);

                // The complete name wins over an equally good component.
                name_score = name_score.max(quality + coverage - 100);
            }
        }

        let exact = text == query || text == compact;
        let scores = Scores {
            text: text_score,
            name: name_score,
            // Initials already compress the name, so omit coverage bonuses.
            #[cfg(feature = "pinyin")]
            initials: quality + i32::from(exact) * 100,
            exact,
        };

        spelling
            .sources
            .iter()
            .map(move |source| (source, scores.rank(source)))
    })
}
