use std::borrow::Cow;
use std::fmt;

/// Represents every type available in the Nitid language.
///
/// Nitid aims to provide a comprehensive set of base types covering
/// integers from 8 to 256 bits (signed and unsigned), floats from 8
/// to 64 bits, three string widths, plus bool and void.
///
/// # Note
/// Many of these types (e.g. I256, F8, F16, String16, String32) are
/// defined in the type system but **do not yet have a runtime
/// implementation** in the generated C code. The transpiler will
/// happily parse and type-check them, but the C backend may produce
/// references to unknown C types.
///
/// # Future work
/// - I256, U256, F8, F16 need software-emulated math or compiler
///   builtins.
/// - String16 / String32 need an actual UTF-16 / UTF-32 runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I8, I16, I32, I64, I128, I256,
    U8, U16, U32, U64, U128, U256,
    F8, F16, F32, F64,
    String, String16, String32,
    Bool, Void,
    /// Array type: element type, and optional compile-time size.
    /// When size is `Some(n)`, the codegen emits a plain C array `type[n]`
    /// instead of a heap-allocated `nitid_array`.
    TyArray(Box<Type>, Option<u64>),
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
            Self::I8 => Cow::Borrowed("int8_t"),
            Self::I16 => Cow::Borrowed("int16_t"),
            Self::I32 => Cow::Borrowed("int"),
            Self::I64 => Cow::Borrowed("int64_t"),
            Self::I128 => Cow::Borrowed("__int128"),
            Self::I256 => Cow::Borrowed("i256"),
            Self::U8 => Cow::Borrowed("uint8_t"),
            Self::U16 => Cow::Borrowed("uint16_t"),
            Self::U32 => Cow::Borrowed("uint32_t"),
            Self::U64 => Cow::Borrowed("uint64_t"),
            Self::U128 => Cow::Borrowed("unsigned __int128"),
            Self::U256 => Cow::Borrowed("u256"),
            Self::F8 => Cow::Borrowed("f8"),
            Self::F16 => Cow::Borrowed("f16"),
            Self::F32 => Cow::Borrowed("float"),
            Self::F64 => Cow::Borrowed("double"),
            Self::String => Cow::Borrowed("nitid_string"),
            Self::String16 => Cow::Borrowed("nitid_string16"),
            Self::String32 => Cow::Borrowed("nitid_string32"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::Void => Cow::Borrowed("void"),
            Self::TyArray(_, None) => Cow::Borrowed("nitid_array"),
            Self::TyArray(elem, Some(_)) => elem.c_str(),
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
