mod description;
mod regular;

pub use description::Grammar;
pub use regular::RegularExpression;

/// Define a constant [`RegularExpression`] compiled from a [`Grammar`].
///
/// `RegularExpression` is sized by the number of states its grammar expands
/// into, so writing one out by hand means naming the grammar three times:
/// once for the size, once for the expansion, and once in the type. This
/// takes the grammar once.
///
/// ```ignore
/// expression! {
///     /// Doc comments and visibility pass through.
///     pub const LINE_EXPR = LINE;
/// }
/// ```
///
/// The definition it produces is a plain `const` item, so this works inside
/// a function body as well as at module level.
///
/// `$grammar` is expanded twice, into the size and into the states, so
/// prefer a named constant over an inline grammar: both expansions are
/// const evaluated either way, but a large grammar written out twice is
/// paid for twice at compile time.
macro_rules! expression {
    (
        $(#[$meta:meta])*
        $vis:vis const $name:ident = $grammar:expr;
    ) => {
        $(#[$meta])*
        $vis const $name: $crate::grammar::RegularExpression<{ $grammar.grammar_size() }> =
            $crate::grammar::RegularExpression::from_grammar(&$grammar);
    };
}

pub(crate) use expression;
