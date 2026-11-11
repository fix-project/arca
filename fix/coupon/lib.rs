#![no_std]

pub const MAX_WITNESSES: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Statement {
    Equivalent = 0,
    Eval = 1,
    Apply = 2,
    Force = 3,
    Think = 4,
    Select = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Exchange {
    Congruence = 0,
    ForceResultEq = 1,
    Think = 2,
    ThinkToForce = 3,
    Eval = 5,
    ForceToEncode = 7,
    EvalEq = 8,
    Symmetry = 10,
    Transitivity = 11,
    Reflexivity = 12,
    ThunkEq = 13,
    ThunkStrictEq = 14,
    ThunkShallowEq = 15,
    EvalResultEq = 20,
    ForceEq = 24,
}

impl Exchange {
    pub const fn statement(self) -> Statement {
        match self {
            Self::Eval | Self::EvalEq => Statement::Eval,
            Self::Think => Statement::Think,
            Self::ThinkToForce | Self::ForceEq => Statement::Force,
            _ => Statement::Equivalent,
        }
    }
}
