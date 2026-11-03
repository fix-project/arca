#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Memoization {
    #[default]
    Auto,
    Always,
    Never,
}

impl core::str::FromStr for Memoization {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err("memoize must be auto, always, or never"),
        }
    }
}

impl core::fmt::Display for Memoization {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        })
    }
}

impl Memoization {
    pub const fn persists(self, completed: bool) -> bool {
        matches!(self, Self::Always) || (matches!(self, Self::Auto) && completed)
    }
}
