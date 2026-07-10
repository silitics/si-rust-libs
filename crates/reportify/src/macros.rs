//! Convenience macros.

/// Define a simple [`Whatever`](crate::Whatever) error type.
///
/// The generated type never carries its own message. Every report built from it needs an
/// explicit description at the call site, via `.whatever("...")`, `bail!("...")`, etc.
///
/// ```
/// reportify::new_whatever_type! {
///     /// Application-level error.
///     pub AppError
/// }
/// ```
#[macro_export]
macro_rules! new_whatever_type {
    ($(#[$meta:meta])* $vis:vis $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy)]
        $vis struct $name(());

        impl $crate::Error for $name {
            fn message(&self) -> ::std::option::Option<&dyn ::std::fmt::Display> {
                ::std::option::Option::None
            }
        }

        impl $crate::Whatever for $name {
            fn new() -> Self {
                Self(())
            }
        }
    };
}

/// Create a freeform report and return it as an error.
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return ::std::result::Result::Err($crate::whatever!($($arg)*))
    };
}

/// Create a freeform report.
#[macro_export]
macro_rules! whatever {
    ($($arg:tt)*) => {
        $crate::Report::whatever(::std::format!($($arg)*))
    };
}

/// Return a freeform report if a condition does not hold.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            $crate::bail!($($arg)*)
        }
    };
}

/// Unwrap a `Result`, returning its error directly, for diverging functions that run
/// forever until something fails: unlike `?`, which needs the enclosing function to
/// return a `Result` (or another [`FromResidual`](std::ops::FromResidual) type), this is
/// for a function whose return type is already the bare error/[`Report`](crate::Report)
/// itself, because it never produces a value, only ever an eventual failure.
///
/// ```
/// use reportify::{Report, ResultExt, new_whatever_type, return_error};
///
/// new_whatever_type! { ServerError }
///
/// /// Stands in for something like `TcpListener::accept`, failing on the third call.
/// fn accept(calls: &mut u32) -> std::io::Result<u32> {
///     *calls += 1;
///     if *calls < 3 {
///         Ok(*calls)
///     } else {
///         Err(std::io::Error::other("connection reset"))
///     }
/// }
///
/// fn run() -> Report<ServerError> {
///     let mut calls = 0;
///     loop {
///         let connection = return_error!(accept(&mut calls).whatever("accept failed"));
///         println!("handling connection {connection}");
///     }
/// }
///
/// let report = run();
/// assert!(format!("{report}").contains("accept failed"));
/// ```
#[macro_export]
macro_rules! return_error {
    ($expr:expr) => {
        match $expr {
            ::std::result::Result::Ok(value) => value,
            ::std::result::Result::Err(error) => return error,
        }
    };
}
