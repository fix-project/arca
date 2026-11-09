#![no_std]

pub const READY: [u8; 8] = *b"ARCAFIX\0";
pub const REQUEST_LEN: usize = 17;

#[macro_export]
macro_rules! benchmark_cases {
    ($($case:ident => ($name:literal, $sized:literal, $millis:literal)),+ $(,)?) => {
        #[derive(Clone, Copy)]
        #[repr(u8)]
        pub enum Operation { $($case),+ }

        impl Operation {
            pub const ALL: &'static [Self] = &[$(Self::$case),+];

            pub fn name(self) -> &'static str {
                match self { $(Self::$case => $name),+ }
            }

            pub fn varies_size(self) -> bool {
                match self { $(Self::$case => $sized),+ }
            }

            pub fn measurement_time(self) -> core::time::Duration {
                core::time::Duration::from_millis(match self { $(Self::$case => $millis),+ })
            }
        }
    };
}
