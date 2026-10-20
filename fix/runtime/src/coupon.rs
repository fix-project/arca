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
    TreeEq = 0,
    ForceResultEq = 1,
    ThinkApplication = 2,
    ThinkToForce = 3,
    EqApplication = 4,
    EvalLeaf = 5,
    EvalTree = 6,
    ForceToEncodeStrict = 7,
    EvalEq = 8,
    EqEncodeStrict = 9,
    Symmetry = 10,
    Transitivity = 11,
    Reflexivity = 12,
    ThunkEq = 13,
    ThunkStrictEq = 14,
    ThunkShallowEq = 15,
    ForceToEncodeShallow = 16,
    EqSelection = 17,
    EqIdentification = 18,
    EqEncodeShallow = 19,
    EvalResultEq = 20,
    TreeRefEq = 21,
    BlobRefEq = 22,
    ForceEq = 24,
    ThinkSelection = 25,
    ThinkIdentification = 26,
}

impl Exchange {
    pub const fn statement(self) -> Statement {
        match self {
            Self::EvalLeaf | Self::EvalTree | Self::EvalEq => Statement::Eval,
            Self::ThinkApplication | Self::ThinkSelection | Self::ThinkIdentification => {
                Statement::Think
            }
            Self::ThinkToForce | Self::ForceEq => Statement::Force,
            _ => Statement::Equivalent,
        }
    }
}
