use std::rc::Rc;

use super::*;

pub struct ConstantConstraint {
    variable: VariableId,
    constant: Value,
}

impl<'a> Constraint<'a> for ConstantConstraint {
    fn variables(&self) -> VariableSet {
        VariableSet::new_singleton(self.variable)
    }

    fn estimate(&self, _variable: VariableId) -> usize {
        1
    }

    fn propose<'b>(&'b self, _variable: VariableId, _binding: Binding) -> Box<dyn Iterator<Item = Value> + 'b>
    where 'a: 'b
    {
        Box::new(std::iter::once(self.constant))
    }

    fn confirm(&self, _variable: VariableId, value: Value, _binding: Binding) -> bool {
        value == self.constant
    }
}