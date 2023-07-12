mod constantconstraint;
mod hashsetconstraint;
mod intersectionconstraint;

use constantconstraint::*;
use hashsetconstraint::*;
use intersectionconstraint::*;

use crate::bitset::ByteBitset;

pub type Value = [u8; 32];
pub type VariableId = u8;
pub type VariableSet = ByteBitset;

#[derive(Copy, Clone)]
pub struct Binding {
    bound: VariableSet,
    values: [Value; 256],
}

impl Binding {
    fn set(&mut self, variable: VariableId, value: Value) {
        self.values[variable as usize] = value;
        self.bound.set(variable);
    }

    fn unset(&mut self, variable: VariableId) {
        self.bound.unset(variable);
    }

    fn get(&self, variable: VariableId) -> Option<Value> {
        if self.bound.is_set(variable) {
            Some(self.values[variable as usize])
        } else {
            None
        }
    }
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            bound: ByteBitset::new_empty(),
            values: [[0; 32]; 256],
        }
    }
}

pub trait Constraint<'a> {
    fn variables(&self) -> VariableSet;
    fn estimate(&self, variable: VariableId) -> usize;
    fn propose<'b>(&'b self, variable: VariableId, binding: Binding) -> Box<dyn Iterator<Item = Value> + 'b>
    where 'a: 'b;
    fn confirm(&self, variable: VariableId, value: Value, binding: Binding) -> bool;
}
struct ConstraintIterator<'a, C: Constraint<'a>> {
    constraint: C,
    binding: Binding,
    variables: VariableSet,
    variable_stack: [u8; 256],
    iterator_stack: [Option<Box<dyn Iterator<Item = Value> + 'a>>; 256],
    stack_depth: isize,
}

impl<'a, C: Constraint<'a>> ConstraintIterator<'a, C> {
    fn new(constraint: C) -> Self {
        let variables = constraint.variables();
        ConstraintIterator {
            constraint,
            binding: Default::default(),
            variables,
            variable_stack: [0; 256],
            iterator_stack: std::array::from_fn(|_| None),
            stack_depth: -1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Search {
    Vertical,
    Horizontal,
    Backtrack,
}

impl<'a, C: Constraint<'a>> Iterator for ConstraintIterator<'a, C> {
    // we will be counting with usize
    type Item = Binding;

    // next() is the only required method
    fn next(&mut self) -> Option<Self::Item> {
        let mut mode = if self.stack_depth == -1 {
            Search::Vertical
        } else {
            Search::Horizontal
        };

        loop {
            match mode {
                Search::Vertical => {
                    if let Some(next_variable) = {
                        let unbound_variables = self.variables.subtract(self.binding.bound);
                        let next_variable = unbound_variables
                            .into_iter()
                            .min_by_key(|v| self.constraint.estimate(*v));
                        next_variable
                    } {
                        self.stack_depth += 1;
                        self.variable_stack[self.stack_depth as usize] = next_variable;
                        self.iterator_stack[self.stack_depth as usize] =
                            Some(self.constraint.propose(next_variable, self.binding));
                        mode = Search::Horizontal;
                    } else {
                        return Some(self.binding.clone());
                    }
                }
                Search::Horizontal => {
                    if let Some(assignment) = self.iterator_stack[self.stack_depth as usize]
                        .as_mut()
                        .unwrap()
                        .next()
                    {
                        self.binding
                            .set(self.variable_stack[self.stack_depth as usize], assignment);
                        mode = Search::Vertical;
                    } else {
                        mode = Search::Backtrack;
                    }
                }
                Search::Backtrack => {
                    self.binding
                        .unset(self.variable_stack[self.stack_depth as usize]);
                    self.iterator_stack[self.stack_depth as usize] = None;
                    self.stack_depth -= 1;
                    if self.stack_depth == -1 {
                        return None;
                    }
                    mode = Search::Vertical;
                }
            }
        }
    }
}
