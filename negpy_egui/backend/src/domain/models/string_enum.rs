//! The `string_enum!` macro: a Python `StrEnum` with its `_missing_` coercion.
//!
//! Saved edits and presets carry whatever the app wrote at the time, so a retired or
//! hand-edited value must degrade to a default instead of failing the load. Each generated
//! enum serializes its canonical spelling and coerces anything else, the way the Python
//! `_missing_` does.

/// One variant per canonical value, an optional alias table, and the fallback variant.
///
/// Generates `as_str`, `coerce`, `Default`, `Display` and the serde impls. `coerce` never
/// fails: a value no variant matches yields the default.
#[macro_export]
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($variant:ident => $lit:literal),+ $(,)? } default $default:ident
    ) => {
        $crate::string_enum!(@gen $(#[$meta])* $name, [$($variant => $lit),+], $default, []);
    };
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($variant:ident => $lit:literal),+ $(,)? }
        aliases { $($alit:literal => $avariant:ident),+ $(,)? } default $default:ident
    ) => {
        $crate::string_enum!(@gen $(#[$meta])* $name, [$($variant => $lit),+], $default, [$($alit => $avariant),+]);
    };
    (@gen $(#[$meta:meta])* $name:ident, [$($variant:ident => $lit:literal),+], $default:ident, [$($alit:literal => $avariant:ident),*]) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),+ }

        impl $name {
            /// The canonical spelling, as the Python enum member's value.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $lit),+ }
            }

            /// The variant for `value`, or the default when nothing matches.
            #[must_use]
            pub fn coerce(value: &str) -> Self {
                match value { $($lit => Self::$variant,)+ $($alit => Self::$avariant,)* _ => Self::$default }
            }
        }

        impl Default for $name {
            fn default() -> Self { Self::$default }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: serde::Serializer {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::coerce(&value))
            }
        }
    };
}
