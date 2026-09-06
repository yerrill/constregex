use crate::description::Grammar;
use crate::list::ConstList;

struct Matcher<const MAX_SIZE: usize> {
    list_1: [bool; MAX_SIZE],
    list_2: [bool; MAX_SIZE],
    list_switch: bool,
}

impl<const MAX_SIZE: usize> Matcher<{ MAX_SIZE }> {
    fn new() -> Self {
        Self {
            list_1: [false; MAX_SIZE],
            list_2: [false; MAX_SIZE],
            list_switch: true,
        }
    }

    fn switch(&mut self) {
        self.list_switch = !self.list_switch;
    }

    fn set_clist(&mut self, i: usize, v: bool) {
        if self.list_switch {
            self.list_1[i] = v;
        } else {
            self.list_2[i] = v;
        }
    }

    fn set_nlist(&mut self, i: usize, v: bool) {
        if self.list_switch {
            self.list_2[i] = v;
        } else {
            self.list_1[i] = v;
        }
    }

    fn get_clist(&self, i: usize) -> bool {
        if self.list_switch {
            self.list_1[i]
        } else {
            self.list_2[i]
        }
    }

    fn get_nlist(&self, i: usize) -> bool {
        if self.list_switch {
            self.list_2[i]
        } else {
            self.list_1[i]
        }
    }
}

/// A partial NFA. Points to the starting index of the first in a chain,
/// and indexs of states which "out" properties are still dangling. Use
/// `dangling()` as the temporary value for those pointers.
#[derive(Clone, Copy)]
struct Fragment<const N: usize> {
    start: usize,
    out_arrows: ConstList<usize, N>,
}

#[derive(Clone, Copy)]
enum Nfa {
    Wild {
        out: usize,
    },
    Literal {
        value: char,
        /// Index of the next state to transition to on a match,
        /// within the fully assembled `states` array.
        out: usize,
    },
    Range {
        /// Bounds of the character range, both ends included.
        start: char,
        end: char,
        out: usize,
    },
    Split {
        left: usize,
        right: usize,
    },
    Match,
}

impl std::fmt::Debug for Nfa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}",
            match self {
                Self::Wild { out } => format!("* {out}"),
                Self::Literal { value, out } => format!("{value} {out}"),
                Self::Range { start, end, out } => format!("{start}-{end} {out}"),
                Self::Split { left, right } => format!("| {left} {right}"),
                Self::Match => "<>".to_string(),
            }
        )
    }
}

type TempStates<const MAX_SIZE: usize> = [Option<Nfa>; MAX_SIZE];

/// Placeholder for an `out` pointer that has not been wired to a real
/// state yet. Deliberately outside the `0..MAX_SIZE` range of valid state
/// indices so it can never be confused for one.
const fn dangling<const MAX_SIZE: usize>() -> usize {
    MAX_SIZE + 1
}

/// Whether `out` is still waiting to be patched to a real state.
const fn is_dangling<const MAX_SIZE: usize>(out: usize) -> bool {
    out >= MAX_SIZE
}

#[derive(Debug)]
pub struct RegularExpression<const MAX_SIZE: usize> {
    start: usize,
    /// Index of the sole `NFA::Match` state. `from_grammar` creates exactly
    /// one, so reaching it is the whole of the accept test.
    match_state: usize,
    states: [Nfa; MAX_SIZE],
}

impl<const MAX_SIZE: usize> RegularExpression<{ MAX_SIZE }> {
    pub const fn from_grammar(input: &Grammar) -> Self {
        let mut temp_states: TempStates<MAX_SIZE> = [None; MAX_SIZE];

        // Expand the grammar, then hang a single `Match` state off every
        // arrow the expansion left dangling.
        let frag = expand_grammar(input, &mut temp_states);

        let match_state = next_slot(&temp_states);
        temp_states[match_state] = Some(Nfa::Match);

        let final_fragment = patch(
            frag,
            Fragment::<MAX_SIZE> {
                start: match_state,
                out_arrows: ConstList::new(),
            },
            &mut temp_states,
        );

        let mut states: [Nfa; MAX_SIZE] = [Nfa::Match; MAX_SIZE];
        let mut i = 0;

        while i < MAX_SIZE {
            // Every slot should be filled. A gap means `grammar_size`
            // over-counted the states this grammar needs.
            let Some(s) = temp_states[i] else {
                panic!("None state in returned array");
            };

            // All pointers in states should point to a valid state
            let oob = match &s {
                Nfa::Wild { out } => is_dangling::<MAX_SIZE>(*out),
                Nfa::Literal { value: _, out } => is_dangling::<MAX_SIZE>(*out),
                Nfa::Range { out, .. } => is_dangling::<MAX_SIZE>(*out),
                Nfa::Split { left, right } => {
                    is_dangling::<MAX_SIZE>(*left) || is_dangling::<MAX_SIZE>(*right)
                }
                Nfa::Match => false,
            };

            if oob {
                panic!("Dangling pointer");
            }

            states[i] = s;

            i += 1;
        }

        Self {
            start: final_fragment.start,
            match_state,
            states,
        }
    }

    /// Add `state` to nlist, following `Split` states as we go so that
    /// nlist only ever holds states that consume a character, or `Match`.
    fn add_nlist(&self, state: usize, m: &mut Matcher<MAX_SIZE>) {
        match self.states[state] {
            Nfa::Split { left, right } => {
                // A split never belongs in the list itself, so its slot is
                // free to double as a "currently being followed" mark. That
                // keeps a loop of splits (e.g. nested `ZeroOrMany`) from
                // recursing forever.
                if m.get_nlist(state) {
                    return;
                }

                m.set_nlist(state, true);
                self.add_nlist(left, m);
                self.add_nlist(right, m);
                m.set_nlist(state, false);
            }
            _ => {
                m.set_nlist(state, true);
            }
        };
    }

    /// Add every state reachable from `state` by consuming `input` to nlist
    fn next_state(&self, state: usize, input: char, m: &mut Matcher<MAX_SIZE>) {
        match self.states[state] {
            Nfa::Wild { out } => self.add_nlist(out, m),
            Nfa::Literal { value, out } if input == value => self.add_nlist(out, m),
            Nfa::Range { start, end, out } if start <= input && input <= end => {
                self.add_nlist(out, m)
            }
            // A literal or range that did not match is a dead end, `Match` has nowhere
            // to go, and splits are resolved as states are added to a list, so
            // an active state is never a split.
            _ => {}
        };
    }

    /// Advance state of NFA by one step, consuming `c`. Returns whether any
    /// state was active to consume it.
    fn step(&self, c: char, m: &mut Matcher<MAX_SIZE>) -> bool {
        let mut did_process_any_state = false;

        // Process each active state from clist
        for i in 0..MAX_SIZE {
            if !m.get_clist(i) {
                continue;
            }

            self.next_state(i, c, m);
            did_process_any_state = true;
            m.set_clist(i, false);
        }

        did_process_any_state
    }

    fn to_matcher(&self) -> Matcher<MAX_SIZE> {
        let mut m = Matcher::new();
        // Seed nlist, then swap it into place so the start state gets the
        // same split-following treatment as every later state.
        self.add_nlist(self.start, &mut m);
        m.switch();
        m
    }

    /// Whether the NFA has arrived at its `Match` state.
    fn is_match(&self, matcher: &Matcher<MAX_SIZE>) -> bool {
        matcher.get_clist(self.match_state)
    }

    pub fn match_whole(&self, input: &str) -> bool {
        let mut m = self.to_matcher();

        for c in input.chars() {
            // If no states were processed in this pass, exit.
            // Every state in nlist is put there by a state that was active in
            // clist, so an empty clist can never refill and no amount of
            // remaining input can produce a match.
            if !self.step(c, &mut m) {
                return false;
            }

            // Swap lists
            m.switch();
        }

        self.is_match(&m)
    }

    /// Whether the expression matches a prefix of `input`. Accepts exactly
    /// what `split_prefix` splits on, so an empty match does not count: the
    /// prefix has to consume at least one character.
    pub fn match_prefix(&self, input: &str) -> bool {
        let mut m = self.to_matcher();

        for c in input.chars() {
            // No state was left alive to consume `c`, so no match can be
            // reached from here and none was reached before now.
            if !self.step(c, &mut m) {
                return false;
            }

            // The states this step produced live in nlist, so swap before
            // asking whether one of them is `Match`.
            m.switch();

            // Unlike `split_prefix` there is no length to report, so the
            // shortest match answers the question and longer ones cannot
            // change it.
            if self.is_match(&m) {
                return true;
            }
        }

        false
    }

    /// Split string on the longest match of the expression at the start of
    /// `input`. The first half is everything the match consumed, the second
    /// is everything after it.
    pub fn split_prefix<'a>(&self, input: &'a str) -> Option<(&'a str, &'a str)> {
        let mut m = self.to_matcher();

        // Byte offset of the end of the match under consideration, and of the
        // longest match seen so far. Counting bytes rather than characters is
        // what keeps `split_at` on a character boundary.
        let mut end = 0;
        let mut longest = None;

        for c in input.chars() {
            // No state was left alive to consume `c`, so no longer match can
            // exist. Whatever was found before this point still stands.
            if !self.step(c, &mut m) {
                break;
            }

            end += c.len_utf8();

            // The states this step produced live in nlist, so swap before
            // asking whether one of them is `Match`.
            m.switch();

            if self.is_match(&m) {
                longest = Some(end);
            }
        }

        longest.map(|end| input.split_at(end))
    }
}

/// Join the out arrows of fragment one to the start of fragment 2.
/// Return a fragment from fragment one start to out arrows of fragment 2.
const fn patch<const MAX_SIZE: usize>(
    fragment_1: Fragment<MAX_SIZE>,
    fragment_2: Fragment<MAX_SIZE>,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let Fragment {
        start: frag_1_start,
        out_arrows: mut frag_1_out,
    } = fragment_1;

    let Fragment {
        start: frag_2_start,
        out_arrows: frag_2_out,
    } = fragment_2;

    while let Some(i) = frag_1_out.pop_front() {
        let mut s = states[i].expect("State given to patch does not exist");

        match s {
            Nfa::Wild { out: _ } => {
                s = Nfa::Wild { out: frag_2_start };
            }
            Nfa::Literal { value, out: _ } => {
                s = Nfa::Literal {
                    value,
                    out: frag_2_start,
                };
            }
            Nfa::Range { start, end, out: _ } => {
                s = Nfa::Range {
                    start,
                    end,
                    out: frag_2_start,
                };
            }
            Nfa::Split { left: l, right: r } => {
                // Only the arms still holding the dangling sentinel get
                // repointed; the other arm is already wired to a real state.
                let new_l = if is_dangling::<MAX_SIZE>(l) {
                    frag_2_start
                } else {
                    l
                };
                let new_r = if is_dangling::<MAX_SIZE>(r) {
                    frag_2_start
                } else {
                    r
                };

                s = Nfa::Split {
                    left: new_l,
                    right: new_r,
                }
            }
            Nfa::Match => panic!("Cannot join match state"),
        };

        states[i] = Some(s);
    }

    Fragment {
        start: frag_1_start,
        out_arrows: frag_2_out,
    }
}

/// Get index of next open slot in states array
///
/// # Panics
///
/// Panics if array is full and no available slot exists
const fn next_slot<const MAX_SIZE: usize>(states: &TempStates<MAX_SIZE>) -> usize {
    let mut i = 0;

    while i < MAX_SIZE {
        if states[i].is_none() {
            return i;
        }
        i += 1;
    }

    panic!("Could not get next slot of states array");
}

/// Route generic grammar to the appropriate function
const fn expand_grammar<const MAX_SIZE: usize>(
    input: &Grammar,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    match input {
        Grammar::Wild => expand_wild(states),
        Grammar::Literal(i) => expand_literal(*i, states),
        Grammar::Range(start, end) => expand_range(*start, *end, states),
        Grammar::Concatenation(grammars) => expand_concatenation(grammars, states),
        Grammar::Alternation(grammars) => expand_alternation(grammars, states),
        Grammar::ZeroOrOne(grammar) => expand_zero_or_one(grammar, states),
        Grammar::ZeroOrMany(grammar) => expand_zero_or_many(grammar, states),
        Grammar::OneOrMany(grammar) => expand_one_or_many(grammar, states),
    }
}

/// Append a wild NFA state to the states list and return a fragment
const fn expand_wild<const MAX_SIZE: usize>(
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let i = next_slot(states);

    states[i] = Some(Nfa::Wild {
        out: dangling::<MAX_SIZE>(),
    });

    let mut l: ConstList<usize, MAX_SIZE> = ConstList::new();

    l.push_back(i);

    Fragment {
        start: i,
        out_arrows: l,
    }
}

/// Append a literal NFA state to the states list and return a fragment
const fn expand_literal<const MAX_SIZE: usize>(
    input: char,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let i = next_slot(states);

    states[i] = Some(Nfa::Literal {
        value: input,
        out: dangling::<MAX_SIZE>(),
    });

    let mut l: ConstList<usize, MAX_SIZE> = ConstList::new();

    l.push_back(i);

    Fragment {
        start: i,
        out_arrows: l,
    }
}

/// Append a range NFA state to the states list and return a fragment
///
/// # Panics
///
/// Panics if the range is empty, i.e. if `end` sorts before `start`.
const fn expand_range<const MAX_SIZE: usize>(
    start: char,
    end: char,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    // An inverted range can never match, so it is always a mistake rather
    // than a deliberate "match nothing".
    assert!(start as u32 <= end as u32);

    let i = next_slot(states);

    states[i] = Some(Nfa::Range {
        start,
        end,
        out: dangling::<MAX_SIZE>(),
    });

    let mut l: ConstList<usize, MAX_SIZE> = ConstList::new();

    l.push_back(i);

    Fragment {
        start: i,
        out_arrows: l,
    }
}

/// Chain a series of grammar pieces together
const fn expand_concatenation<const MAX_SIZE: usize>(
    input: &[Grammar<'_>],
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let input_len = input.len();

    assert!(input_len > 0);

    // Expand first grammar, returning fragment
    let mut fragment_1 = expand_grammar(&input[0], states);
    let mut fragment_2;

    let mut i = 1;

    // Repeatedly expand next grammar
    while i < input_len {
        // Expand next grammar
        fragment_2 = expand_grammar(&input[i], states);

        // Point all remaining arrows from previous grammar at start of next
        fragment_1 = patch(fragment_1, fragment_2, states);

        i += 1;
    }

    // Return encapsulation of start to end of last outbound arrows
    fragment_1
}

/// Expand every branch, then chain splits in front of them so entering the
/// fragment enters all of the branches at once
const fn expand_alternation<const MAX_SIZE: usize>(
    input: &[Grammar],
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let input_len = input.len();

    assert!(input_len > 0);

    let mut starts: ConstList<usize, MAX_SIZE> = ConstList::new();
    let mut out_arrows: ConstList<usize, MAX_SIZE> = ConstList::new();

    // Expand each grammar in input
    let mut i = 0;
    while i < input_len {
        let Fragment {
            start: s,
            out_arrows: o,
        } = expand_grammar(&input[i], states);

        starts.push_back(s);
        out_arrows.extend(o);

        i += 1;
    }

    // Alternating between `n` branches takes `n - 1` splits: one branch needs
    // none, two need a single split, and each branch past that chains another
    // split onto the previous one.

    // Get first grammar pointer
    let Some(g1) = starts.pop_front() else {
        unreachable!();
    };

    // Get second grammar pointer (or return)
    let Some(g2) = starts.pop_front() else {
        // Only single grammar fragment to return
        return Fragment {
            start: g1,
            out_arrows,
        };
    };

    // Add split for first two grammars
    let mut split = next_slot(states);

    states[split] = Some(Nfa::Split {
        left: g1,
        right: g2,
    });

    // Repeatedly add split and +1 grammar for remaining
    while let Some(g) = starts.pop_front() {
        let new_split = next_slot(states);

        states[new_split] = Some(Nfa::Split {
            left: split,
            right: g,
        });

        split = new_split;
    }

    // Return fragment of start = last split and all out arrows of grammars
    Fragment {
        start: split,
        out_arrows,
    }
}

/// `?` - a split in front of the grammar, whose second arm skips past it.
/// The skipping arm dangles, so it leaves the fragment alongside the
/// grammar's own arrows
const fn expand_zero_or_one<const MAX_SIZE: usize>(
    input: &Grammar,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let Fragment {
        start: input_start,
        mut out_arrows,
    } = expand_grammar(input, states);

    let split = next_slot(states);

    states[split] = Some(Nfa::Split {
        left: input_start,
        right: dangling::<MAX_SIZE>(),
    });

    out_arrows.push_back(split);

    Fragment {
        start: split,
        out_arrows,
    }
}

/// `*` - as `?`, but the grammar's arrows loop back to the split rather than
/// leaving the fragment, so the split is the only way out
const fn expand_zero_or_many<const MAX_SIZE: usize>(
    input: &Grammar,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let grammar_frag = expand_grammar(input, states);

    let split = next_slot(states);

    states[split] = Some(Nfa::Split {
        left: grammar_frag.start,
        right: dangling::<MAX_SIZE>(),
    });

    let mut out: ConstList<usize, MAX_SIZE> = ConstList::new();

    out.push_back(split);

    let split_frag = Fragment {
        start: split,
        out_arrows: out,
    };

    let rev = patch(grammar_frag, split_frag, states);

    Fragment {
        start: split,
        ..rev
    }
}

/// `+` - the same loop as `*`, but entered at the grammar instead of the
/// split, so one pass through it is mandatory
const fn expand_one_or_many<const MAX_SIZE: usize>(
    input: &Grammar,
    states: &mut TempStates<MAX_SIZE>,
) -> Fragment<MAX_SIZE> {
    let grammar_frag = expand_grammar(input, states);

    let split = next_slot(states);

    states[split] = Some(Nfa::Split {
        left: grammar_frag.start,
        right: dangling::<MAX_SIZE>(),
    });

    let mut out: ConstList<usize, MAX_SIZE> = ConstList::new();

    out.push_back(split);

    let split_frag = Fragment {
        start: split,
        out_arrows: out,
    };

    patch(grammar_frag, split_frag, states)
}

#[cfg(test)]
mod tests {
    use crate::{
        description::{Grammar, Grammar::*},
        expression,
    };

    const A: Grammar<'_> = Literal('A');
    const B: Grammar<'_> = Literal('B');
    const C: Grammar<'_> = Literal('C');

    const CR: Grammar<'_> = Literal('\r');
    const LF: Grammar<'_> = Literal('\n');
    const SPACE: Grammar<'_> = Literal(' ');
    const TAB: Grammar<'_> = Literal('\t');

    /// Compiles `$grammar` and asserts that every string under `accepts`
    /// matches it in its entirety, and that every string under `rejects` does
    /// not.
    ///
    /// Building the expression is a test in itself: it happens during const
    /// evaluation, so a grammar whose `grammar_size` disagrees with the number
    /// of states it actually expands into fails to compile rather than fails
    /// at runtime.
    macro_rules! check {
        (
            $grammar:expr,
            accepts: [$($accept:expr),* $(,)?],
            rejects: [$($reject:expr),* $(,)?] $(,)?
        ) => {{
            const G: Grammar<'_> = $grammar;
            expression!(const R = G;);

            $(assert!(
                R.match_whole($accept),
                "expected {:?} to match\nstates: {:?}",
                $accept,
                R
            );)*

            $(assert!(
                !R.match_whole($reject),
                "expected {:?} not to match\nstates: {:?}",
                $reject,
                R
            );)*
        }};
    }

    #[test]
    fn literal() {
        check!(A, accepts: ["A"], rejects: ["", "B", "AA", "a"]);
    }

    #[test]
    fn wild_matches_exactly_one_character_of_any_kind() {
        check!(
            Wild,
            accepts: ["A", "z", "7", " ", "\n", "é", "🦀"],
            rejects: ["", "AB", "éé"],
        );
    }

    #[test]
    fn literals_compare_whole_characters_not_bytes() {
        // "é" and "🦀" are two and four bytes; matching walks `char`s, so a
        // partial encoding of one can never satisfy a literal.
        check!(
            Concatenation(&[Literal('é'), Literal('🦀')]),
            accepts: ["é🦀"],
            rejects: ["", "é", "🦀", "e🦀", "é🦀🦀"],
        );
    }

    #[test]
    fn range_matches_one_character_between_its_inclusive_bounds() {
        check!(
            Range('b', 'd'),
            accepts: ["b", "c", "d"],
            rejects: ["", "a", "e", "B", "bc"],
        );
    }

    #[test]
    fn range_of_a_single_character_behaves_like_a_literal() {
        check!(Range('A', 'A'), accepts: ["A"], rejects: ["", "@", "B", "AA"]);
    }

    #[test]
    fn range_bounds_are_code_points_not_bytes() {
        // The bounds span one, two and four byte encodings, so comparing
        // anything other than whole `char`s would order these wrongly.
        check!(
            Range('\u{0000}', char::MAX),
            accepts: ["A", " ", "\n", "é", "🦀"],
            rejects: ["", "é🦀"],
        );
    }

    #[test]
    fn range_can_exclude_characters_by_spanning_around_them() {
        // "any character other than CR or LF", the shape every line-oriented
        // rule in the markdown grammar is built from.
        check!(
            Alternation(&[
                Range('\u{0000}', '\u{0009}'),
                Range('\u{000B}', '\u{000C}'),
                Range('\u{000E}', char::MAX),
            ]),
            accepts: ["a", "\t", "\u{000C}", "🦀"],
            rejects: ["", "\n", "\r"],
        );
    }

    #[test]
    fn concatenation() {
        check!(
            Concatenation(&[A, B]),
            accepts: ["AB"],
            rejects: ["", "A", "B", "BA", "ABA", "AAB"],
        );
    }

    #[test]
    fn concatenation_of_one_is_the_inner_grammar() {
        check!(Concatenation(&[A]), accepts: ["A"], rejects: ["", "B", "AA"]);
    }

    #[test]
    fn concatenation_around_a_wild() {
        check!(
            Concatenation(&[A, Wild, A]),
            accepts: ["ACA", "AAA", "A A"],
            rejects: ["", "AA", "AC", "ACAA", "BCA"],
        );
    }

    #[test]
    fn alternation() {
        check!(
            Alternation(&[A, B]),
            accepts: ["A", "B"],
            rejects: ["", "C", "AA", "AB"],
        );
    }

    #[test]
    fn alternation_of_one_is_the_inner_grammar() {
        check!(Alternation(&[A]), accepts: ["A"], rejects: ["", "B", "AA"]);
    }

    #[test]
    fn alternation_of_three_chains_splits() {
        check!(
            Alternation(&[A, B, C]),
            accepts: ["A", "B", "C"],
            rejects: ["", "D", "AB", "CC"],
        );
    }

    #[test]
    fn alternation_nested_in_alternation() {
        check!(
            Alternation(&[Alternation(&[A, B]), C]),
            accepts: ["A", "B", "C"],
            rejects: ["", "D", "AC"],
        );
    }

    #[test]
    fn alternation_branches_of_different_lengths() {
        // The shorter branch reaches `Match` while the longer one is still
        // running, so both have to stay live at the same time.
        check!(
            Alternation(&[Concatenation(&[A, B]), A]),
            accepts: ["A", "AB"],
            rejects: ["", "B", "ABA", "AA"],
        );
    }

    #[test]
    fn zero_or_one() {
        check!(
            ZeroOrOne(&A),
            accepts: ["", "A"],
            rejects: ["AA", "B", "BA"],
        );
    }

    #[test]
    fn zero_or_one_of_concatenation() {
        check!(
            ZeroOrOne(&Concatenation(&[A, B])),
            accepts: ["", "AB"],
            rejects: ["A", "B", "ABAB"],
        );
    }

    #[test]
    fn zero_or_many() {
        check!(
            ZeroOrMany(&A),
            accepts: ["", "A", "AA", "AAA"],
            rejects: ["B", "AAB", "BAA", "ABA"],
        );
    }

    #[test]
    fn zero_or_many_of_concatenation() {
        check!(
            ZeroOrMany(&Concatenation(&[A, B])),
            accepts: ["", "AB", "ABAB", "ABABAB"],
            rejects: ["A", "BA", "ABA", "ABB"],
        );
    }

    #[test]
    fn zero_or_many_of_wild_accepts_anything() {
        check!(
            ZeroOrMany(&Wild),
            accepts: ["", "A", "hello", "line\nline", "🦀🦀"],
            rejects: [],
        );
    }

    #[test]
    fn one_or_many() {
        check!(
            OneOrMany(&A),
            accepts: ["A", "AA", "AAA"],
            rejects: ["", "B", "AAB", "BAA"],
        );
    }

    #[test]
    fn one_or_many_of_concatenation() {
        check!(
            OneOrMany(&Concatenation(&[A, B])),
            accepts: ["AB", "ABAB"],
            rejects: ["", "A", "ABA", "BAB"],
        );
    }

    #[test]
    fn nested_zero_or_many() {
        // Nesting repetition puts a split in a loop with another split. The
        // matcher has to finish following those without consuming a character,
        // rather than chasing the loop forever.
        check!(
            ZeroOrMany(&ZeroOrMany(&A)),
            accepts: ["", "A", "AAA"],
            rejects: ["B", "AB"],
        );
    }

    #[test]
    fn one_or_many_nested_in_zero_or_many() {
        check!(
            ZeroOrMany(&OneOrMany(&A)),
            accepts: ["", "A", "AAA"],
            rejects: ["B", "AB"],
        );
    }

    #[test]
    fn repetition_of_a_nullable_grammar() {
        // `A?` already matches nothing, so repeating it adds a second way to
        // pass through the fragment without consuming anything.
        check!(
            OneOrMany(&ZeroOrOne(&A)),
            accepts: ["", "A", "AA"],
            rejects: ["B", "AB"],
        );
    }

    #[test]
    fn zero_or_many_inside_alternation() {
        check!(
            Alternation(&[ZeroOrMany(&A), B]),
            accepts: ["", "A", "AAA", "B"],
            rejects: ["AB", "BB", "BA"],
        );
    }

    #[test]
    fn concatenation_of_repetitions() {
        check!(
            Concatenation(&[A, ZeroOrMany(&B), Wild]),
            accepts: ["AZ", "ABBBZ", "ABB", "AB"],
            rejects: ["", "A", "BZ"],
        );
    }

    #[test]
    fn adjacent_repetitions_of_the_same_literal() {
        // `A*A` forces the matcher to hold both the looping branch and the
        // trailing literal at once, since it cannot know which `A` is the last.
        check!(
            Concatenation(&[ZeroOrMany(&A), A]),
            accepts: ["A", "AA", "AAA"],
            rejects: ["", "B", "AAB"],
        );
    }

    #[test]
    fn long_input_that_dies_early_is_still_rejected() {
        // `match_whole` bails as soon as no state is left alive; the remaining
        // input must not resurrect one.
        check!(
            Concatenation(&[A, B]),
            accepts: ["AB"],
            rejects: ["ZZZZZZZZZZZZZZZZZZZZ", "ABABABABABABABABABAB"],
        );
    }

    /// `(CR LF | LF | CR)`, the line ending rule the markdown grammar is
    /// built on. A composite of every operator worth exercising together.
    const EOL: Grammar<'_> = Alternation(&[Concatenation(&[CR, LF]), LF, CR]);

    #[test]
    fn end_of_line() {
        check!(
            EOL,
            accepts: ["\r\n", "\n", "\r"],
            rejects: ["", "\n\r", "\r\r", "\n\n", "a"],
        );
    }

    #[test]
    fn blank_line() {
        check!(
            Concatenation(&[ZeroOrMany(&Alternation(&[SPACE, TAB])), EOL]),
            accepts: ["\n", "\r\n", " \n", "\t\r\n", " \t \r"],
            rejects: ["", " ", "a\n", " a \n", "\n\n"],
        );
    }

    #[test]
    fn line() {
        check!(
            Concatenation(&[ZeroOrMany(&Wild), EOL]),
            accepts: ["\n", "abc\n", "abc\r\n", "abc\r", "a\nb\n", "🦀\n"],
            rejects: ["", "abc"],
        );
    }

    /// Compiles `$grammar` and asserts that `split_prefix` returns
    /// `$expected` for each `$input`. Every returned pair is also checked to
    /// be a true split of its input: the two halves must join back into the
    /// original string, so a prefix can never gain, lose or reorder bytes.
    /// `match_prefix` is checked against the same answer, since it is the
    /// same match reported as a boolean.
    macro_rules! check_split {
        (
            $grammar:expr,
            $($input:expr => $expected:expr),* $(,)?
        ) => {{
            const G: Grammar<'_> = $grammar;
            expression!(const R = G;);

            $({
                let input: &str = $input;
                let expected: Option<(&str, &str)> = $expected;
                let actual = R.split_prefix(input);

                assert_eq!(
                    actual, expected,
                    "split_prefix({:?})\nstates: {:?}",
                    input, R
                );

                if let Some((prefix, rest)) = actual {
                    assert_eq!(
                        format!("{prefix}{rest}"), input,
                        "halves of split_prefix({:?}) do not rejoin",
                        input
                    );
                }

                assert_eq!(
                    R.match_prefix(input), actual.is_some(),
                    "match_prefix({:?}) disagrees with split_prefix",
                    input
                );
            })*
        }};
    }

    #[test]
    fn split_prefix_on_a_literal() {
        check_split!(
            A,
            "A" => Some(("A", "")),
            "AB" => Some(("A", "B")),
            "AAA" => Some(("A", "AA")),
        );
    }

    #[test]
    fn split_prefix_returns_none_when_the_prefix_does_not_match() {
        // The expression is anchored at the start of `input`: a match later in
        // the string is not a prefix, so it is not found.
        check_split!(
            A,
            "" => None,
            "B" => None,
            "BA" => None,
            "BAAAA" => None,
        );
    }

    #[test]
    fn split_prefix_of_a_concatenation_consumes_the_whole_match() {
        check_split!(
            Concatenation(&[A, B]),
            "AB" => Some(("AB", "")),
            "ABC" => Some(("AB", "C")),
            "ABAB" => Some(("AB", "AB")),
            "A" => None,
            "AC" => None,
        );
    }

    #[test]
    fn split_prefix_splits_on_character_boundaries() {
        // `enumerate` counts characters while `split_at` counts bytes, so a
        // multi-byte character ahead of the split point is the case that tells
        // the two apart. Splitting inside one panics.
        check_split!(
            Concatenation(&[Wild, Wild]),
            "🦀🦀é" => Some(("🦀🦀", "é")),
            "é🦀" => Some(("é🦀", "")),
            "aé!" => Some(("aé", "!")),
        );
    }

    #[test]
    fn split_prefix_takes_the_longest_match() {
        // `A+` reaches `Match` on the first `A` and again on every one after
        // it. The split is taken at the last of them, not the first.
        check_split!(
            OneOrMany(&A),
            "A" => Some(("A", "")),
            "AA" => Some(("AA", "")),
            "AAB" => Some(("AA", "B")),
            "AABAA" => Some(("AA", "BAA")),
            "B" => None,
        );
    }

    #[test]
    fn split_prefix_keeps_the_longest_match_when_a_later_one_dies() {
        // `AB?` matches after `A`, stays alive for a longer match, then dies
        // on `C`. The shorter match it already found is still the answer.
        check_split!(
            Concatenation(&[A, ZeroOrOne(&B)]),
            "AB" => Some(("AB", "")),
            "AC" => Some(("A", "C")),
            "ACB" => Some(("A", "CB")),
            "ABB" => Some(("AB", "B")),
        );
    }

    #[test]
    fn split_prefix_of_a_nullable_grammar_still_consumes_a_character() {
        // `A*` matches the empty string, but `split_prefix` only tests for a
        // match after consuming a character, so it never reports a zero-length
        // prefix. An input that cannot consume one has no split at all.
        check_split!(
            ZeroOrMany(&A),
            "A" => Some(("A", "")),
            "AB" => Some(("A", "B")),
            "" => None,
            "B" => None,
        );
    }

    #[test]
    fn split_prefix_with_alternate_branches_of_different_lengths() {
        // Both branches match `AB`; the longer one decides the split.
        check_split!(
            Alternation(&[Concatenation(&[A, B]), A]),
            "AB" => Some(("AB", "")),
            "ABB" => Some(("AB", "B")),
            "AC" => Some(("A", "C")),
            "B" => None,
        );
    }

    #[test]
    fn split_prefix_takes_a_line_off_the_front_of_a_document() {
        // The use the function exists for: peel one line, keep the rest.
        check_split!(
            Concatenation(&[ZeroOrMany(&Alternation(&[
                Range('\u{0000}', '\u{0009}'),
                Range('\u{000B}', '\u{000C}'),
                Range('\u{000E}', char::MAX),
            ])), EOL]),
            "abc\ndef\n" => Some(("abc\n", "def\n")),
            // The `CR` branch of `EOL` is satisfied by the `\r` alone, but the
            // `CR LF` branch is still alive and matches one character later.
            // Taking the longest keeps a CRLF ending whole.
            "abc\r\ndef" => Some(("abc\r\n", "def")),
            "abc\r" => Some(("abc\r", "")),
            "\nabc" => Some(("\n", "abc")),
            "🦀🦀\nabc" => Some(("🦀🦀\n", "abc")),
            "abc" => None,
            "" => None,
        );
    }

    #[test]
    fn split_prefix_does_not_panic_on_input_that_kills_every_state() {
        // Nothing is left alive to consume the tail, and reading past the end
        // of the string would be an out of bounds split.
        check_split!(
            Concatenation(&[A, B]),
            "ZZZZZZZZZZZZZZZZZZZZ" => None,
            "AZZZZZZZZZZZZZZZZZZZ" => None,
            "🦀🦀🦀🦀🦀🦀🦀🦀" => None,
        );
    }

    // `grammar_size` has to be exact: too small and `next_slot` runs out of
    // room, too large and `from_grammar` finds a slot it never filled. Both
    // panic during const evaluation, so every `check!` above already proves
    // the count for its own grammar. These pin down the formula itself.
    const _: () = assert!(A.grammar_size() == 2);
    const _: () = assert!(Wild.grammar_size() == 2);
    const _: () = assert!(Range('a', 'z').grammar_size() == 2);
    const _: () = assert!(Concatenation(&[A, B, C]).grammar_size() == 4);
    const _: () = assert!(Alternation(&[A, B, C]).grammar_size() == 6);
    const _: () = assert!(ZeroOrOne(&A).grammar_size() == 3);
    const _: () = assert!(ZeroOrMany(&A).grammar_size() == 3);
    const _: () = assert!(OneOrMany(&A).grammar_size() == 3);
    const _: () = assert!(EOL.grammar_size() == 7);
}
