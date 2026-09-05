pub use crate::prelude::*;

pub use oxc_allocator::Allocator;
pub use oxc_ast::ast::{
    CallExpression, Expression, ImportExpression, TSImportEqualsDeclaration, TSImportType,
    TSModuleReference,
};
pub use oxc_ast_visit::{Visit, walk};
pub use oxc_parser::{ParseOptions, Parser, ParserReturn};
pub use oxc_span::SourceType;
pub use self_cell::self_cell;
