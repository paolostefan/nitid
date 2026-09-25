use std::borrow::Cow;
use std::fmt;

/// Represents every type available in the Nitid language.
///
/// Nitid aims to provide a comprehensive set of base types covering
/// integers from 8 to 256 bits (signed and unsigned), floats from 8
/// to 64 bits, three string widths, plus bool and void.
///
/// # Note
/// Some of these types (e.g. I256, F8, F16, String16, String32) are
/// defined in the type system but **do not yet have a runtime
/// implementation** in the generated C code. The transpiler will
/// happily parse and type-check them, but the C backend may produce
/// references to unknown C types.
///
/// # Note on MSVC(R)
/// MSVC(R) does not support 128-bit integer types, in particular math operations.
/// As of Nitid v0.2.0, 128-bit integers are only available for other compilers.
/// This is done via macros in [types.h](/runtime/types.h).
///
/// # Future work
/// - Add advanced primitives like e.g. maps.
/// - I256, U256, F8, F16 need software-emulated math or compiler
///   builtins.
#[derive(Debug, Clone, PartialEq, Ord, Eq, PartialOrd)]
pub enum Type {
    Bool,
    Void,
    I8,
    I16,
    I32,
    I64,
    I128,
    I256, // unused
    U8,
    U16,
    U32,
    U64,
    U128,
    U256, // unused
    F8,   // unused
    F16,  // unused
    F32,
    F64,
    String,
    String16,
    String32,
    /// Array type: element type, and optional compile-time size.
    ///
    /// Dynamically-sized arrays are heap-allocated (`nitid_array`) and
    /// can be resized at runtime with `.resize()`. A declared size is
    /// only the initial length; the array still lives on the heap.
    /// When size is `None` the length comes from the initializer.
    TyArray(Box<Type>, Option<u64>),
    /// Fixed-size array (`fixed` keyword): emitted as a plain C array
    /// `type[n]`. It cannot be resized.
    TyFixedArray(Box<Type>, u64),
    // Pointer
    TyPtr(Box<Type>, bool), // (*T, is_mutable)
    /// Named struct type (user-defined).
    Struct(String),
    /// Named enum type (user-defined).
    Enum(String),
}

impl Type {
    /// Parse a type name from a Nitid source string.
    ///
    /// Accepts both the canonical names (`i32`, `u64`, `f64`, …) and
    /// C-style aliases (`int`, `float`, `double`).
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" | "int" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "i256" => Some(Self::I256),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "u256" => Some(Self::U256),
            "f8" => Some(Self::F8),
            "f16" => Some(Self::F16),
            "f32" | "float" => Some(Self::F32),
            "f64" | "double" => Some(Self::F64),
            "string" => Some(Self::String),
            "string16" => Some(Self::String16),
            "string32" => Some(Self::String32),
            "bool" => Some(Self::Bool),
            "void" => Some(Self::Void),
            _ => None,
        }
    }

    /// Return the C type string emitted for this Nitid type.
    ///
    /// This is what the codegen module uses when generating variable
    /// declarations and function signatures.
    pub fn c_str(&self) -> Cow<'static, str> {
        match self {
            Self::I8 => Cow::Borrowed("i8"),
            Self::I16 => Cow::Borrowed("i16"),
            Self::I32 => Cow::Borrowed("i32"),
            Self::I64 => Cow::Borrowed("i64"),
            Self::I128 => Cow::Borrowed("i128"),
            Self::I256 => Cow::Borrowed("i256"),
            Self::U8 => Cow::Borrowed("u8"),
            Self::U16 => Cow::Borrowed("u16"),
            Self::U32 => Cow::Borrowed("u32"),
            Self::U64 => Cow::Borrowed("u64"),
            Self::U128 => Cow::Borrowed("u128"),
            Self::U256 => Cow::Borrowed("u256"),
            Self::F8 => Cow::Borrowed("f8"),
            Self::F16 => Cow::Borrowed("f16"),
            Self::F32 => Cow::Borrowed("f32"),
            Self::F64 => Cow::Borrowed("f64"),
            Self::String => Cow::Borrowed("nitid_string"),
            Self::String16 => Cow::Borrowed("nitid_string16"),
            Self::String32 => Cow::Borrowed("nitid_string32"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::Void => Cow::Borrowed("void"),
            Self::TyArray(..) => Cow::Borrowed("nitid_array"),
            Self::TyFixedArray(elem, _) => elem.c_str(),
            Self::TyPtr(elem, mutable) => {
                if *mutable {
                    elem.c_str() + " *"
                } else {
                    elem.c_str() + " *const"
                }
            }
            Self::Struct(name) => Cow::Owned(name.clone()),
            Self::Enum(name) => Cow::Owned(name.clone()),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.c_str())
    }
}

/// Return the element type of an array type, or None if not an array.
pub fn array_elem_type(t: &Type) -> Option<&Type> {
    match t {
        Type::TyArray(elem, _) => Some(elem.as_ref()),
        _ => None,
    }
}

/// Check if a type is one of the three string types.
pub fn is_string_type(t: &Type) -> bool {
    matches!(t, Type::String | Type::String16 | Type::String32)
}
