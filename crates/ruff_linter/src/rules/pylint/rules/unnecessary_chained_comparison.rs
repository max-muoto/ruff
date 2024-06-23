use std::collections::{HashMap, HashSet};

use crate::checkers::ast::Checker;
use ruff_diagnostics::{Diagnostic, Violation};
use ruff_macros::{derive_message_formats, violation};
use ruff_python_ast::{self as ast};

/// ## What it does
/// Checks for boolean operations such as `a < b and b < c`
/// that can be refactored into a single comparison `a < b < c`.
///
/// ## Why is this bad?
/// A single comparison is semantically clearer and more concise.
///
/// ## Example
/// ```python
/// a = int(input())
/// b = int(input())
/// c = int(input())
/// if a < b and b < c:
///     pass
/// ```
///
/// Use instead:
/// ```python
/// a = int(input())
/// b = int(input())
/// c = int(input())
/// if a < b < c:
///     pass
/// ```

#[violation]
pub struct UnnecessaryChainedComparison;

impl Violation for UnnecessaryChainedComparison {
    #[derive_message_formats]
    fn message(&self) -> String {
        format!("Simplified chain comparison exists between the operands.")
    }
}

// Each integer is a unique identifier for the node.
#[derive(Default)]
struct Bounds {
    lower_bound: HashSet<u32>,
    upper_bound: HashSet<u32>,
}

fn update_bounds<'a>(
    operator: ast::CmpOp,
    id: &'a str,
    node_idx: u32,
    is_left: bool,
    uses: &mut HashMap<&'a str, Bounds>,
) {
    match operator {
        ast::CmpOp::Lt | ast::CmpOp::LtE if is_left => {
            uses.entry(id).or_default().lower_bound.insert(node_idx);
        }
        ast::CmpOp::Gt | ast::CmpOp::GtE if is_left => {
            uses.entry(id).or_default().upper_bound.insert(node_idx);
        }
        ast::CmpOp::Lt | ast::CmpOp::LtE if !is_left => {
            uses.entry(id).or_default().upper_bound.insert(node_idx);
        }
        ast::CmpOp::Gt | ast::CmpOp::GtE if !is_left => {
            uses.entry(id).or_default().lower_bound.insert(node_idx);
        }
        _ => {}
    }
}

fn set_lower_upper_bounds<'a>(
    node: &'a ast::ExprCompare,
    uses: &mut HashMap<&'a str, Bounds>,
    node_idx: u32,
) {
    let mut left_operand: &ast::Expr = &node.left;
    for (right_operand, operator) in node.comparators.iter().zip(node.ops.iter()) {
        if let Some(left_name_expr) = left_operand.as_name_expr() {
            update_bounds(*operator, &left_name_expr.id, node_idx, true, uses);
        }

        if let Some(right_name_expr) = right_operand.as_name_expr() {
            update_bounds(*operator, &right_name_expr.id, node_idx, false, uses);
        }

        left_operand = right_operand;
    }
}

/// PLR1716
pub(crate) fn unnecessary_chained_comparison(checker: &mut Checker, bool_op: &ast::ExprBoolOp) {
    let ast::ExprBoolOp { op, values, range } = bool_op;

    if *op != ast::BoolOp::And || values.len() < 2 {
        return;
    }

    let mut uses: HashMap<&str, Bounds> = HashMap::new();

    let mut node_idx: u32 = 0;
    for expr in values {
        let Some(compare_expr) = expr.as_compare_expr() else {
            continue;
        };
        set_lower_upper_bounds(compare_expr, &mut uses, node_idx);
        node_idx += 1;
    }

    for bound in uses.values() {
        let num_shared = bound.lower_bound.intersection(&bound.upper_bound).count();
        if num_shared < bound.lower_bound.len() && num_shared < bound.upper_bound.len() {
            let diagnostic = Diagnostic::new(UnnecessaryChainedComparison, *range);
            checker.diagnostics.push(diagnostic);
            break;
        }
    }
}
