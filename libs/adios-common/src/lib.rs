use std::{fmt::Display, rc::Rc};

use bilge::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub start: u64,
    pub end: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub ts: Option<Timestamp>,
    pub apndp: bool,
    pub rnw: bool,
    /// 2nd and 3rd bit
    /// Real `a` would be `Self::a << 2`
    pub a: u2,
    pub data: u32,
}

impl From<Command> for Input {
    fn from(value: Command) -> Self {
        Self::Command(value)
    }
}

#[derive(Clone)]
pub enum Input {
    /// To inject something display'able into the VM stepping
    Landmark { metadata: Rc<dyn Display> },
    /// Actual command pushing the VM forward
    Command(Command),
}
