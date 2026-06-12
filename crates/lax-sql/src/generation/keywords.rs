/// Words that start a new top level clause line in a statement.
///
/// Statement leading words like `create`, `alter`, `drop`, and `delete` are
/// deliberately absent: they also appear inside constructs like
/// `ON DELETE SET NULL`, `ALTER COLUMN x DROP NOT NULL`, or
/// `CREATE OR REPLACE`, where a line break would be wrong, and as the first
/// word of a statement they never need a break anyway. `set` only starts a
/// clause in an `update` statement, handled separately.
pub const CLAUSE_STARTERS: &[&str] = &[
  "except",
  "from",
  "group",
  "having",
  "insert",
  "intersect",
  "join",
  "limit",
  "offset",
  "order",
  "returning",
  "select",
  "union",
  "values",
  "where",
  "window",
  "with",
];

/// A clause starter is suppressed when it directly follows one of these
/// words, which covers `ON UPDATE CASCADE`, `ON DELETE SET NULL`, and
/// upsert `DO UPDATE SET` style constructs.
pub const STARTER_SUPPRESSORS: &[&str] = &["do", "on"];

/// Words that start a clause only when they lead into a `JOIN`.
pub const JOIN_PREFIXES: &[&str] = &["cross", "full", "inner", "lateral", "left", "natural", "outer", "right"];

/// Reserved and near universal SQL keywords, used only for the optional
/// keyword case transform. Quoted identifiers and function names are never
/// part of this list by construction, since they are different token kinds.
pub const KEYWORDS: &[&str] = &[
  "add",
  "all",
  "alter",
  "and",
  "as",
  "asc",
  "begin",
  "between",
  "by",
  "cascade",
  "case",
  "check",
  "column",
  "commit",
  "constraint",
  "create",
  "cross",
  "current_date",
  "current_time",
  "current_timestamp",
  "database",
  "default",
  "delete",
  "desc",
  "distinct",
  "drop",
  "else",
  "end",
  "escape",
  "except",
  "exists",
  "false",
  "fetch",
  "filter",
  "first",
  "for",
  "foreign",
  "from",
  "full",
  "group",
  "having",
  "if",
  "ilike",
  "in",
  "index",
  "inner",
  "insert",
  "intersect",
  "into",
  "is",
  "join",
  "key",
  "last",
  "lateral",
  "left",
  "like",
  "limit",
  "natural",
  "not",
  "null",
  "nulls",
  "offset",
  "on",
  "only",
  "or",
  "order",
  "outer",
  "over",
  "partition",
  "primary",
  "recursive",
  "references",
  "rename",
  "returning",
  "right",
  "rollback",
  "row",
  "rows",
  "select",
  "set",
  "table",
  "then",
  "to",
  "transaction",
  "true",
  "truncate",
  "union",
  "unique",
  "update",
  "using",
  "values",
  "view",
  "when",
  "where",
  "window",
  "with",
];

pub fn is_clause_starter(word: &str) -> bool {
  contains_ignore_case(CLAUSE_STARTERS, word)
}

pub fn is_starter_suppressor(word: &str) -> bool {
  contains_ignore_case(STARTER_SUPPRESSORS, word)
}

pub fn is_join_prefix(word: &str) -> bool {
  contains_ignore_case(JOIN_PREFIXES, word)
}

pub fn is_keyword(word: &str) -> bool {
  contains_ignore_case(KEYWORDS, word)
}

fn contains_ignore_case(list: &[&str], word: &str) -> bool {
  list.iter().any(|k| k.eq_ignore_ascii_case(word))
}
