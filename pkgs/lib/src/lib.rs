//! Declaration-driven Vitest inventories. Test bodies remain user-owned source text.

mod declarations;
mod inventory;
mod sync;

pub use declarations::parse_declarations;
pub use inventory::{Options, Report, generate};
pub use sync::synchronize;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A stable describe path; overloads and declaration merges share one node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suite {
    #[serde(default, skip_serializing_if = "is_false")]
    pub callable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, Suite>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Suite {
    pub fn at(&mut self, path: &[String]) -> &mut Self {
        let mut node = self;
        for name in path {
            node = node.children.entry(name.clone()).or_default();
        }
        node
    }

    fn merge(&mut self, other: &Self) {
        self.callable |= other.callable;
        for (name, child) in &other.children {
            self.children.entry(name.clone()).or_default().merge(child);
        }
    }

    fn prune(&mut self) {
        self.children.retain(|_, child| {
            child.prune();
            child.callable || !child.children.is_empty()
        });
    }
}

pub(crate) fn parse<'a>(
    allocator: &'a oxc_allocator::Allocator,
    source: &'a str,
    declaration: bool,
) -> anyhow::Result<oxc_ast::ast::Program<'a>> {
    let parsed = oxc_parser::Parser::new(
        allocator,
        source,
        oxc_span::SourceType::ts().with_typescript_definition(declaration),
    )
    .parse();
    anyhow::ensure!(
        !parsed.panicked && parsed.diagnostics.is_empty(),
        "TypeScript parse failed: {:?}",
        parsed.diagnostics
    );
    Ok(parsed.program)
}
