use std::cmp::Reverse;
use std::ops::Range;

use smallvec::{SmallVec, smallvec};

#[derive(Clone, Copy)]
pub enum MatchMode {
    Full,
    Prefix,
    Substring,
}

pub fn edit_distance(
    query: &[char],
    candidate: &[char],
    max_edits: u16,
    mode: MatchMode,
) -> Option<(u16, Range<usize>)> {
    let max_edits = usize::from(max_edits).min(query.len() / 3);

    if query.len() > candidate.len() + max_edits
        || (matches!(mode, MatchMode::Full) && candidate.len() > query.len() + max_edits)
    {
        return None;
    }

    match mode {
        MatchMode::Full => calculate::<false>(query, candidate, max_edits, candidate.len()),
        MatchMode::Prefix => calculate::<false>(
            query,
            candidate,
            max_edits,
            query.len().saturating_sub(max_edits),
        ),
        MatchMode::Substring => calculate::<true>(query, candidate, max_edits, 0),
    }
}

// Specialize the two paths so anchored matching does not track span starts.
fn calculate<const FREE_START: bool>(
    query: &[char],
    candidate: &[char],
    max_edits: usize,
    first_end: usize,
) -> Option<(u16, Range<usize>)> {
    // Anchored matches visit only the diagonal band. Substrings can start
    // anywhere, so they need every column.
    let width = if FREE_START {
        candidate.len() + 1
    } else {
        candidate.len().min(query.len() + max_edits) + 1
    };
    let unreachable = max_edits + 1;

    // Query and field limits (512 and 4,096 chars) fit in u16. Cells contain
    // (edits, start); ties keep the earlier start and therefore longer span.
    let mut first: SmallVec<[_; 32]> = (0..width)
        .map(|column| {
            if FREE_START {
                (0_u16, column as u16)
            } else {
                (column.min(unreachable) as u16, 0)
            }
        })
        .collect();
    let mut second = first.clone();
    let mut third: SmallVec<[(u16, u16); 32]> = smallvec![(unreachable as u16, 0); width];
    let (mut previous_two, mut previous, mut current) = (
        first.as_mut_slice(),
        second.as_mut_slice(),
        third.as_mut_slice(),
    );

    let advance =
        |(edits, start): (u16, u16), cost| (edits + cost, if FREE_START { start } else { 0 });

    for i in 1..=query.len() {
        let (start, end) = if FREE_START {
            (1, width - 1)
        } else {
            (
                i.saturating_sub(max_edits).max(1),
                (i + max_edits).min(width - 1),
            )
        };
        current[start - 1] = ((if start == 1 { i } else { unreachable }) as u16, 0);

        if end + 1 < width {
            current[end + 1] = (unreachable as u16, 0);
        }

        let mut row_min = unreachable as u16;

        for j in start..=end {
            let substitution =
                advance(previous[j - 1], u16::from(query[i - 1] != candidate[j - 1]));
            let deletion = advance(previous[j], 1);
            let insertion = advance(current[j - 1], 1);
            current[j] = substitution.min(deletion).min(insertion);

            if i > 1
                && j > 1
                && query[i - 1] == candidate[j - 2]
                && query[i - 2] == candidate[j - 1]
            {
                let transposition = advance(previous_two[j - 2], 1);
                current[j] = current[j].min(transposition);
            }

            row_min = row_min.min(current[j].0);
        }

        if usize::from(row_min) > max_edits {
            return None;
        }

        std::mem::swap(&mut previous_two, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }

    (first_end..width)
        .filter_map(|end| {
            let (edits, start) = previous[end];
            let edits = usize::from(edits);
            let start = if FREE_START { usize::from(start) } else { 0 };
            let evidence_budget = query.len().min(end - start) / 3;

            (edits <= max_edits && edits <= evidence_budget).then_some((edits as u16, start..end))
        })
        .min_by_key(|(edits, span)| (*edits, Reverse(span.len()), span.start))
}
