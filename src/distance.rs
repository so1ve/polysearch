#[derive(Clone, Copy)]
pub enum MatchLength {
    Full,
    Prefix,
}

#[must_use]
pub fn damerau_levenshtein(
    left: &[char],
    right: &[char],
    max_distance: u16,
    length: MatchLength,
) -> Option<(u16, usize)> {
    let max_distance = usize::from(max_distance).min(left.len() / 3);

    if left.len() > right.len() + max_distance
        || (matches!(length, MatchLength::Full) && right.len() > left.len() + max_distance)
    {
        return None;
    }

    // Only endpoints within the edit budget can match. Untyped suffixes do
    // not increase the matrix width, and only the diagonal band is visited.
    let width = right.len().min(left.len() + max_distance) + 1;
    let unreachable = max_distance + 1;
    let mut previous_two: Vec<_> = (0..width).map(|j| j.min(unreachable)).collect();
    let mut previous = previous_two.clone();
    let mut current = vec![unreachable; width];

    for i in 1..=left.len() {
        let start = i.saturating_sub(max_distance).max(1);
        let end = (i + max_distance).min(width - 1);
        current[start - 1] = if start == 1 { i } else { unreachable };

        if end + 1 < width {
            current[end + 1] = unreachable;
        }

        let mut row_min = unreachable;

        for j in start..=end {
            let substitution = previous[j - 1] + usize::from(left[i - 1] != right[j - 1]);
            current[j] = substitution.min(previous[j] + 1).min(current[j - 1] + 1);

            if i > 1 && j > 1 && left[i - 1] == right[j - 2] && left[i - 2] == right[j - 1] {
                current[j] = current[j].min(previous_two[j - 2] + 1);
            }

            row_min = row_min.min(current[j]);
        }

        if row_min > max_distance {
            return None;
        }

        std::mem::swap(&mut previous_two, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }

    let start = match length {
        MatchLength::Full => right.len(),
        MatchLength::Prefix => left.len().saturating_sub(max_distance),
    };

    (start..width)
        .filter_map(|end| {
            let distance = previous[end];
            // A prefix is a complete explanation of the user's input.
            // Untyped characters provide neither errors nor extra evidence.
            let evidence_budget = left.len().min(end) / 3;

            (distance <= max_distance && distance <= evidence_budget)
                .then_some((distance as u16, end))
        })
        .min_by_key(|&(edits, end)| (edits, std::cmp::Reverse(end)))
}
