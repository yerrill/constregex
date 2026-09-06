# constregex

A small regular expression engine that compiles entirely in `const` context.

A pattern written with `constregex` is checked and turned into a state
machine at compile time, not at runtime. There is no separate build step and
no external pattern syntax to parse: the pattern is Rust code, so a broken
one is a compile error instead of a runtime panic.

> **This is an experiment**, not a production-ready regex crate. It exists
> to explore how far `const fn` can go towards building a regex engine with
> no runtime cost and no `unsafe`. It may grow further, but there's no
> promise of that — see [Status](#status).

## Why

Most regex crates parse a pattern string and build the matcher the first
time it is used. `constregex` does that work during compilation instead:

- **No runtime compilation cost.** The matcher already exists as a `const`
  value by the time the program runs.
- **No syntax to typo.** Patterns are built from an enum, not a string, so
  the compiler checks them like any other Rust expression.
- **Usable in `const` and `static` contexts**, including `#![no_std]`-style
  code that cannot allocate.
- **Composition without string concatenation.** A `Grammar` is a plain Rust
  value, so it can be built up from smaller named pieces and reused across
  several larger patterns. Compiling (`expression!`) is a separate step you
  choose to take on whichever piece you actually need as a matcher — see
  [Composition](#composition).

The trade-off is expressiveness: `constregex` supports the core of regular
expressions (literals, ranges, concatenation, alternation, and repetition),
not the full syntax of crates like [`regex`](https://docs.rs/regex).

## Usage

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
constregex = "0.1"
```

Describe a pattern with [`Grammar`], then compile it into a
[`RegularExpression`] with the [`expression!`] macro:

```rust
use constregex::{expression, Grammar};
use constregex::Grammar::*;

// "cat" or "dog"
const PET: Grammar<'_> = Alternation(&[
    Concatenation(&[Literal('c'), Literal('a'), Literal('t')]),
    Concatenation(&[Literal('d'), Literal('o'), Literal('g')]),
]);

expression! {
    const PET_EXPR = PET;
}

assert!(PET_EXPR.match_whole("cat"));
assert!(PET_EXPR.match_whole("dog"));
assert!(!PET_EXPR.match_whole("fish"));
```

`expression!` exists because a hand-written `RegularExpression` needs its
size parameter, which is derived from the grammar itself
(`RegularExpression<{ PET.grammar_size() }>`). The macro takes the grammar
once and fills in the size and the expansion for you.

### Grammar

A [`Grammar`] is built from these cases:

| Variant | Meaning | Regex equivalent |
| --- | --- | --- |
| `Wild` | any single character | `.` |
| `Literal(c)` | the character `c` | `c` |
| `Range(a, b)` | any character in `a..=b`, inclusive | `[a-b]` |
| `Concatenation(&[..])` | each grammar in order | `ab` |
| `Alternation(&[..])` | any one of the grammars | `a\|b` |
| `ZeroOrOne(g)` | `g`, zero or one times | `g?` |
| `ZeroOrMany(g)` | `g`, zero or more times | `g*` |
| `OneOrMany(g)` | `g`, one or more times | `g+` |

Grammars nest freely, so larger patterns are built up from smaller named
constants, as in the `PET` example above.

### Composition

A `Grammar` is just data, so sub-patterns can be named and reused the way
any other Rust value can, and combined into more than one larger pattern
without ever being compiled on their own:

```rust
use constregex::{expression, Grammar};
use constregex::Grammar::*;

// End of line: CRLF, LF, or a lone CR.
const EOL: Grammar<'_> = Alternation(&[
    Concatenation(&[Literal('\r'), Literal('\n')]),
    Literal('\n'),
    Literal('\r'),
]);

// Reuses EOL as-is.
const BLANK_LINE: Grammar<'_> = Concatenation(&[
    ZeroOrMany(&Alternation(&[Literal(' '), Literal('\t')])),
    EOL,
]);

// Also reuses EOL, in a different pattern.
const LINE: Grammar<'_> = Concatenation(&[ZeroOrMany(&Wild), EOL]);

// EOL itself is never compiled — only the patterns built from it are.
expression! { const BLANK_LINE_EXPR = BLANK_LINE; }
expression! { const LINE_EXPR = LINE; }
```

`EOL` is never compiled by itself here — only `BLANK_LINE` and `LINE` are,
each with `expression!`, at the point each is actually needed as a matcher.
Building a regex string this way means repeating (and correctly escaping)
the same substring everywhere it is used; here it is one constant, checked
once by the compiler, referenced from as many patterns as needed.

### Matching

A compiled [`RegularExpression`] exposes three ways to test it against a
string:

- `match_whole(input)` — does the pattern match the entire string?
- `match_prefix(input)` — does the pattern match some prefix of the string?
- `split_prefix(input)` — split `input` on the longest matching prefix,
  returning `(matched, rest)`, or `None` if no prefix matches.

```rust
use constregex::{expression, Grammar};
use constregex::Grammar::*;

const DIGIT: Grammar<'_> = Range('0', '9');
const DIGITS: Grammar<'_> = OneOrMany(&DIGIT);

expression! {
    const DIGITS_EXPR = DIGITS;
}

assert_eq!(DIGITS_EXPR.split_prefix("123abc"), Some(("123", "abc")));
assert!(DIGITS_EXPR.match_prefix("123abc"));
assert!(!DIGITS_EXPR.match_whole("123abc"));
```

## How it works

Each `Grammar` expands into an [NFA](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton)
(nondeterministic finite automaton), following the construction described in
Russ Cox's [Regular Expression Matching Can Be Simple And Fast](https://swtch.com/~rsc/regexp/regexp1.html).
`Grammar::grammar_size` counts, at compile time, exactly how many states
that expansion needs, so `RegularExpression` can be a fixed-size array with
no over-allocation and no heap. Matching then runs the standard Thompson NFA
simulation: at each input character, every currently active state is
advanced in step, so the whole match runs in time proportional to the
length of the input and the size of the pattern, with no exponential
backtracking.

## Status

This project started as an experiment in how much of a regex engine can be
built as `const fn`, and it's still mostly that: a proof of concept rather
than a crate meant for production use. The core engine (literals, ranges,
concatenation, alternation, and repetition) is implemented and tested, but
there's no committed roadmap beyond it, no stable `1.0` API, and no promise
that the API will stay as it is. It may grow into something more complete
if that seems worthwhile later.

## License

Licensed under the [MIT license](LICENSE).
