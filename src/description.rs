pub enum Grammar<'a> {
    /// Match any single character
    Wild,
    /// Match a sequence of characters in order
    Literal(char),
    /// Match any single character in the inclusive range `start..=end`
    Range(char, char),
    /// Match a sequence of grammars in order
    Concatenation(&'a [Grammar<'a>]),
    /// Match on any of the given grammars
    Alternation(&'a [Grammar<'a>]),
    // ? Match Grammar zero or more times
    ZeroOrOne(&'a Grammar<'a>),
    // * Match Grammar zero or more times
    ZeroOrMany(&'a Grammar<'a>),
    // ? Match Grammar one or more times
    OneOrMany(&'a Grammar<'a>),
}

impl Grammar<'_> {
    /// `const` time calculation of the required number of states for a given grammar.
    /// Designed so `RegularExpression` can be given an exact size parameter.
    #[must_use]
    pub const fn grammar_size(&self) -> usize {
        rec_calculate_size(self) + 1
    }
}

/// Recursively calculate states size.
/// Based on `RegularExpression`.
const fn rec_calculate_size(input: &Grammar) -> usize {
    match input {
        Grammar::Wild | Grammar::Literal(_) | Grammar::Range(_, _) => 1,
        Grammar::Concatenation(grammars) => {
            let mut i = 0;
            let mut acc = 0;

            while i < grammars.len() {
                acc += rec_calculate_size(&grammars[i]);
                i += 1;
            }

            acc
        }
        Grammar::Alternation(grammars) => {
            assert!(!grammars.is_empty(), "Alternation must have at least one branch");

            let mut i = 0;
            let mut acc = 0;

            while i < grammars.len() {
                acc += rec_calculate_size(&grammars[i]);
                i += 1;
            }

            acc += grammars.len() - 1;

            acc
        }
        Grammar::ZeroOrOne(grammar) | Grammar::ZeroOrMany(grammar) | Grammar::OneOrMany(grammar) => {
            rec_calculate_size(grammar) + 1
        }
    }
}
