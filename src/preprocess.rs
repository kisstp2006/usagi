//! Tiny Lua source preprocessor for compound-assignment convenience
//! operators. Stock Lua 5.4 has no `+=`/`-=`/etc.; we rewrite them to
//! plain `LHS = LHS op (RHS)` before the chunk hits `lua.load()`.
//!
//! The rewrite is intentionally line-anchored, matching PICO-8's
//! preprocessor: a line whose body parses as `<indent> LHS op= RHS
//! [trailing comment]` becomes `<indent> LHS = LHS op (RHS) [trailing
//! comment]`. Any other shape (compound op inside `if cond then ... end`,
//! inside a `for` body, on the RHS of another expression, etc.) is left
//! alone, and the user keeps writing stock Lua.
//!
//! Long strings (`[[ ... ]]`, `[==[ ... ]==]`) and block comments
//! (`--[[ ... ]]`, `--[==[ ... ]==]`) can span lines, so a per-line
//! lexer state machine tracks "are we currently inside one of those?"
//! and refuses to rewrite anything until we're back in code mode.
//!
//! Known limitation: the LHS is duplicated verbatim, same as PICO-8 —
//! `t[f()] += 1` evaluates `f()` twice. Documented; if anyone hits it
//! they can rewrite the line by hand.

/// Operators we rewrite. The text is matched as-is at the source level
/// once the LHS has been consumed and trailing whitespace skipped.
const COMPOUND_OPS: &[(&str, &str)] = &[
    ("+=", "+"),
    ("-=", "-"),
    ("*=", "*"),
    ("/=", "/"),
    ("%=", "%"),
];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum LexState {
    Code,
    /// Inside a `--[==[ ... ]==]` block comment with `level` `=` signs.
    BlockComment(usize),
    /// Inside a `[==[ ... ]==]` long-string literal with `level` `=` signs.
    LongString(usize),
}

/// Rewrites compound-assignment lines in `src`. Bytes that aren't valid
/// UTF-8 are returned unchanged so the Lua parser produces the real
/// error rather than us masking it.
pub fn preprocess(src: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(src) else {
        return src.to_vec();
    };
    let mut out = String::with_capacity(text.len() + 32);
    let mut state = LexState::Code;
    for raw_line in text.split_inclusive('\n') {
        let (body, nl) = split_trailing_newline(raw_line);
        let rewritten = if state == LexState::Code {
            try_rewrite_compound_line(body)
        } else {
            None
        };
        match rewritten {
            Some(s) => out.push_str(&s),
            None => out.push_str(body),
        }
        out.push_str(nl);
        state = advance_lex_state(state, body);
    }
    out.into_bytes()
}

fn split_trailing_newline(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix("\r\n") {
        (stripped, &line[stripped.len()..])
    } else if let Some(stripped) = line.strip_suffix('\n') {
        (stripped, &line[stripped.len()..])
    } else {
        (line, "")
    }
}

/// Attempts to rewrite a single line that is *entirely* in code mode at
/// its start. Returns Some(new_line_body) on a successful match, None
/// otherwise (in which case the caller emits the line unchanged).
///
/// The match shape is:
///   <indent> <LHS> <ws>* <op>= <ws>* <rhs> [trailing_short_comment]?
///
/// where LHS is `name (.name | [bracket_balanced])*` and `rhs` is the
/// remainder of the line up to a trailing `--` short comment (or end).
/// Long-string starts (`[[`, `[=[`, ...) and block-comment starts
/// (`--[[`) inside the rhs disqualify the rewrite, since splicing them
/// inside the generated parens would change their parse.
fn try_rewrite_compound_line(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let indent_end = bytes.iter().position(|c| !is_horiz_ws(*c))?;
    let lhs_end = scan_lhs(bytes, indent_end)?;
    let mut i = lhs_end;
    while i < bytes.len() && is_horiz_ws(bytes[i]) {
        i += 1;
    }
    let (op_text, plain_op) = match_compound_op(&bytes[i..])?;
    let after_op = i + op_text.len();
    let mut j = after_op;
    while j < bytes.len() && is_horiz_ws(bytes[j]) {
        j += 1;
    }
    let rhs_start = j;

    // Find where the RHS ends: at a trailing short comment (`--` not
    // followed by `[[` / `[=[`...) or end of line. Inside the RHS we
    // can't accept a long-string or block-comment start, since those
    // could change parse when we wrap the rhs in parens.
    let (rhs_end, trailing) = locate_rhs_end(bytes, rhs_start)?;

    let indent = &line[..indent_end];
    let lhs = &line[indent_end..lhs_end];
    let rhs = line[rhs_start..rhs_end].trim_end_matches(is_horiz_ws_char);
    if rhs.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(line.len() + lhs.len() + 8);
    out.push_str(indent);
    out.push_str(lhs);
    out.push_str(" = ");
    out.push_str(lhs);
    out.push(' ');
    out.push_str(plain_op);
    out.push_str(" (");
    out.push_str(rhs);
    out.push(')');
    out.push_str(trailing);
    Some(out)
}

/// Walks an LHS starting at `start`, returning the byte index just past
/// it, or None if the bytes there don't form a valid simple lvalue
/// (`name`, `name.name`, `name[expr]`, `name.foo[bar].baz`, etc.).
///
/// Function-call segments like `f()` and method calls like `t:m()`
/// aren't allowed — neither is a valid assignment target by itself, and
/// admitting them just adds parse complexity without buying real cases.
fn scan_lhs(s: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    if i >= s.len() || !is_ident_start(s[i]) {
        return None;
    }
    i += 1;
    while i < s.len() && is_ident_cont(s[i]) {
        i += 1;
    }
    loop {
        if i + 1 < s.len() && s[i] == b'.' && s[i + 1] != b'.' {
            // .name segment. The `s[i + 1] != b'.'` guard prevents
            // chewing into a `..` concat operator that happens to follow
            // an identifier (e.g. `s = a..b`).
            i += 1;
            if i >= s.len() || !is_ident_start(s[i]) {
                return None;
            }
            i += 1;
            while i < s.len() && is_ident_cont(s[i]) {
                i += 1;
            }
        } else if i < s.len() && s[i] == b'[' {
            // Bracket-balanced segment. Strings inside aren't tokenized
            // (a `]` inside a `"..."` would confuse us); typical game
            // code uses numeric or identifier indexes, which are safe.
            let mut depth: i32 = 1;
            i += 1;
            while i < s.len() && depth > 0 {
                match s[i] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            if depth != 0 {
                return None;
            }
        } else {
            break;
        }
    }
    Some(i)
}

/// At byte position 0 of `s`, returns `(matched_text, plain_op)` if one
/// of the compound operators starts there. `plain_op` is what we splice
/// into the rewritten line (e.g. `+=` → `+`).
fn match_compound_op(s: &[u8]) -> Option<(&'static str, &'static str)> {
    for (text, plain) in COMPOUND_OPS {
        if s.starts_with(text.as_bytes()) {
            return Some((text, plain));
        }
    }
    None
}

/// Walks the line from `start` and returns `(rhs_end, trailing)` where
/// `trailing` is a trailing short-comment (`-- ...`) we need to preserve
/// outside the parentheses. Returns None when the RHS contains a long-
/// string opener (`[[`, `[=[`, ...) or a block-comment opener
/// (`--[[`, `--[=[`, ...) — splicing those inside parens could change
/// the program's parse.
fn locate_rhs_end(s: &[u8], start: usize) -> Option<(usize, &str)> {
    let mut i = start;
    let mut in_short_string: Option<u8> = None;
    while i < s.len() {
        let c = s[i];
        if let Some(q) = in_short_string {
            if c == b'\\' && i + 1 < s.len() {
                i += 2;
                continue;
            }
            if c == q {
                in_short_string = None;
            }
            i += 1;
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_short_string = Some(c);
            i += 1;
            continue;
        }
        if c == b'[' && long_bracket_level(&s[i..]).is_some() {
            // `[[`, `[=[`, ...: starts a long string. Refuse to rewrite.
            return None;
        }
        if c == b'-' && i + 1 < s.len() && s[i + 1] == b'-' {
            // Comment. If it's `--[[` or `--[==[`, it's a block comment;
            // refuse. Otherwise it's a short comment running to EOL.
            if i + 2 < s.len() && s[i + 2] == b'[' && long_bracket_level(&s[i + 2..]).is_some() {
                return None;
            }
            // Trailing short comment kept outside the parentheses so it
            // doesn't end up inside the generated `(...)`.
            let comment_start = trim_horiz_ws_left(s, i, start);
            let comment = std::str::from_utf8(&s[comment_start..]).ok()?;
            return Some((comment_start, comment));
        }
        i += 1;
    }
    if in_short_string.is_some() {
        // Unterminated string literal — let the Lua parser report it.
        return None;
    }
    Some((s.len(), ""))
}

/// Walks back from `from` skipping trailing horizontal whitespace, but
/// never past `floor`. Used when peeling a trailing `-- ...` comment off
/// the rhs so we don't carry the indentation between rhs and `--` into
/// the generated parentheses.
fn trim_horiz_ws_left(s: &[u8], from: usize, floor: usize) -> usize {
    let mut k = from;
    while k > floor && is_horiz_ws(s[k - 1]) {
        k -= 1;
    }
    k
}

/// If `s` starts with a long-bracket opener (`[`, `[=`, `[==`, ..., `[`),
/// returns Some(level) where level is the number of `=` signs. Otherwise
/// None. Used both for long-string detection and for `--[[` block
/// comments.
fn long_bracket_level(s: &[u8]) -> Option<usize> {
    if s.is_empty() || s[0] != b'[' {
        return None;
    }
    let mut k = 1;
    while k < s.len() && s[k] == b'=' {
        k += 1;
    }
    if k < s.len() && s[k] == b'[' {
        Some(k - 1)
    } else {
        None
    }
}

/// Single-line lex pass to update the across-line state. Walks through
/// short strings and short comments (which always end on the same line),
/// and tracks entry/exit of long strings and block comments.
fn advance_lex_state(start: LexState, line: &str) -> LexState {
    let s = line.as_bytes();
    let mut state = start;
    let mut i = 0;
    while i < s.len() {
        match state {
            LexState::Code => {
                let c = s[i];
                if c == b'-' && i + 1 < s.len() && s[i + 1] == b'-' {
                    // Comment of some kind.
                    if let Some(level) = long_bracket_level(&s[i + 2..]) {
                        state = LexState::BlockComment(level);
                        i += 2 + level + 2;
                        continue;
                    }
                    // Short comment: rest of line is comment, but state
                    // stays Code for the next line.
                    return LexState::Code;
                }
                if c == b'"' || c == b'\'' {
                    // Short string. Skip past matching quote on this line.
                    let q = c;
                    i += 1;
                    while i < s.len() && s[i] != q {
                        if s[i] == b'\\' && i + 1 < s.len() {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    if i < s.len() {
                        i += 1;
                    }
                    continue;
                }
                if c == b'['
                    && let Some(level) = long_bracket_level(&s[i..])
                {
                    state = LexState::LongString(level);
                    i += level + 2;
                    continue;
                }
                i += 1;
            }
            LexState::BlockComment(level) | LexState::LongString(level) => {
                if s[i] == b']' {
                    let mut k = i + 1;
                    let mut count = 0;
                    while k < s.len() && s[k] == b'=' {
                        count += 1;
                        k += 1;
                    }
                    if count == level && k < s.len() && s[k] == b']' {
                        state = LexState::Code;
                        i = k + 1;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    state
}

#[inline]
fn is_horiz_ws(c: u8) -> bool {
    c == b' ' || c == b'\t'
}
#[inline]
fn is_horiz_ws_char(c: char) -> bool {
    c == ' ' || c == '\t'
}
#[inline]
fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}
#[inline]
fn is_ident_cont(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pp(src: &str) -> String {
        String::from_utf8(preprocess(src.as_bytes())).unwrap()
    }

    #[test]
    fn rewrites_basic_arithmetic_compounds() {
        assert_eq!(pp("x += 1\n"), "x = x + (1)\n");
        assert_eq!(pp("x -= 2\n"), "x = x - (2)\n");
        assert_eq!(pp("x *= 3\n"), "x = x * (3)\n");
        assert_eq!(pp("x /= 4\n"), "x = x / (4)\n");
        assert_eq!(pp("x %= 5\n"), "x = x % (5)\n");
    }

    #[test]
    fn preserves_indentation() {
        assert_eq!(pp("  x += 1\n"), "  x = x + (1)\n");
        assert_eq!(pp("\tx += 1\n"), "\tx = x + (1)\n");
    }

    #[test]
    fn handles_dotted_and_indexed_lhs() {
        assert_eq!(pp("t.score += 10\n"), "t.score = t.score + (10)\n");
        assert_eq!(pp("a[1] += 5\n"), "a[1] = a[1] + (5)\n");
        assert_eq!(pp("a[i].x += dt\n"), "a[i].x = a[i].x + (dt)\n");
    }

    #[test]
    fn keeps_trailing_short_comment_outside_parens() {
        assert_eq!(pp("x += 1 -- bump\n"), "x = x + (1) -- bump\n");
        assert_eq!(
            pp("x += 1  -- two spaces\n"),
            "x = x + (1)  -- two spaces\n"
        );
    }

    #[test]
    fn rhs_can_be_a_full_expression() {
        assert_eq!(pp("x += a + b * c\n"), "x = x + (a + b * c)\n");
        // Wrapping in parens prevents precedence surprises.
        assert_eq!(pp("x *= a + b\n"), "x = x * (a + b)\n");
    }

    #[test]
    fn does_not_rewrite_inside_short_string() {
        // The += sits in a string literal on the LEFT of an assignment;
        // not a compound assignment statement at all.
        assert_eq!(pp("s = \"x += 1\"\n"), "s = \"x += 1\"\n");
    }

    #[test]
    fn does_not_rewrite_inside_block_comment() {
        let input = "--[[\nx += 1\n]]\ny = 0\n";
        let expected = "--[[\nx += 1\n]]\ny = 0\n";
        assert_eq!(pp(input), expected);
    }

    #[test]
    fn does_not_rewrite_inside_long_string() {
        let input = "s = [[\nx += 1\n]]\n";
        assert_eq!(pp(input), input);
    }

    #[test]
    fn does_not_rewrite_when_compound_is_not_at_statement_position() {
        // `if cond then x += 1 end` is not a line-anchored compound
        // statement; we leave it for the user to write longhand.
        assert_eq!(pp("if c then x += 1 end\n"), "if c then x += 1 end\n");
    }

    #[test]
    fn refuses_when_rhs_contains_long_string_opener() {
        // Splicing `[[...]]` inside generated parens could change parse.
        assert_eq!(pp("s += [[hi]]\n"), "s += [[hi]]\n");
    }

    #[test]
    fn long_block_comment_with_levels_resumes_code_after_close() {
        let input = "--[==[\nx += 1\n]==]\ny += 1\n";
        let expected = "--[==[\nx += 1\n]==]\ny = y + (1)\n";
        assert_eq!(pp(input), expected);
    }

    #[test]
    fn pass_through_on_invalid_utf8() {
        let bad = vec![b'x', b' ', b'+', b'=', b' ', 0xff, b'\n'];
        // Not valid UTF-8 → preprocessor returns input unchanged.
        assert_eq!(preprocess(&bad), bad);
    }

    #[test]
    fn preserves_crlf_line_endings() {
        assert_eq!(pp("x += 1\r\n"), "x = x + (1)\r\n");
    }

    #[test]
    fn empty_rhs_is_left_alone() {
        // `x +=` with nothing after — let the Lua parser error rather
        // than emit a malformed `x = x + ()`.
        assert_eq!(pp("x +=\n"), "x +=\n");
    }
}
