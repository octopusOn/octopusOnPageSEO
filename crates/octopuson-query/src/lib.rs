//! Filter AST, text query language, query planner and safe SQL compilation.
//!
//! - [`Query`] is the syntax tree shared by the text language ([`parse`] / [`to_dsl`]) and
//!   the desktop's visual builder (as JSON).
//! - [`validate`] performs semantic checks.
//! - [`sql::compile`] plans a query onto a rollup or raw hits and emits parameterized SQL.
//! - [`template`] holds the built-in reports.

mod ast;
pub mod dsl;
mod field;
mod measure;
mod shape;
pub mod sql;
pub mod template;
mod validate;

pub use ast::{CompareOp, Condition, Expr, Literal, Predicate, Query, SortDirection, SortKey};
pub use dsl::{ParseError, ParseErrorKind, parse, to_dsl};
pub use field::{Field, FieldKind};
pub use measure::{Dimension, Metric};
pub use shape::{ROW_COLUMNS, Shape};
pub use validate::{MAX_LAST_DAYS, MAX_LIMIT, ValidationError, validate, validate_predicate};
