use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::hash::Hash;

use derive_new::new;

use super::{Loc, TypeDef, TypeDefBody};
use crate::utils;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeBody {
    Bool,
    // FIXME: use interface/trait for this
    AnyNumber,
    // FIXME: use interface/trait for this
    AnySignedNumber,
    // FIXME: use interface/trait for this
    AnyFloat,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    USize,
    F32,
    F64,
    Inferred(InferredType),
    String(StringType),
    Array(ArrayType),
    TypeRef(usize),
}

#[derive(Debug, Clone, new)]
pub struct Type {
    pub body: TypeBody,
    pub loc: Option<Loc>,
}

macro_rules! unordered {
    ($a:pat, $b:pat) => {
        ($a, $b) | ($b, $a)
    };
}
macro_rules! number {
    ($var:ident $( , $gen:ident)*) => {
        unordered!(
            Type { body: TypeBody::$var, loc: _ },
            Type { body: TypeBody::AnyNumber $( | TypeBody::$gen )*, loc: _ })
    };
}
macro_rules! body {
    ($pat:pat) => {
        Type { body: $pat, loc: _ }
    };
}
impl Type {
    pub fn unknown(loc: Option<Loc>) -> Self {
        Type::new(
            TypeBody::Inferred(InferredType {
                properties: utils::SortedMap::new(),
            }),
            loc,
        )
    }

    pub fn is_unknown(&self) -> bool {
        if let TypeBody::Inferred(i) = &self.body {
            return i.properties.is_empty();
        }
        false
    }

    pub fn is_inferred(&self) -> bool {
        matches!(&self.body, TypeBody::Inferred(_))
    }

    pub fn is_composite(&self) -> bool {
        matches!(&self.body, TypeBody::String(_) | TypeBody::Array(_))
    }

    pub fn is_primitive(&self) -> bool {
        self.is_bool() || self.is_number()
    }

    pub fn is_bool(&self) -> bool {
        matches!(&self.body, TypeBody::Bool)
    }

    pub fn is_number(&self) -> bool {
        matches!(&self.body, TypeBody::AnyNumber | TypeBody::AnySignedNumber)
            || self.is_sint()
            || self.is_uint()
            || self.is_float()
    }

    pub fn is_int(&self) -> bool {
        self.is_sint() || self.is_uint()
    }

    pub fn is_sint(&self) -> bool {
        matches!(
            &self.body,
            TypeBody::I8 | TypeBody::I16 | TypeBody::I32 | TypeBody::I64
        )
    }

    pub fn is_uint(&self) -> bool {
        matches!(
            &self.body,
            TypeBody::U8
                | TypeBody::U16
                | TypeBody::U32
                | TypeBody::U64
                | TypeBody::USize
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(
            &self.body,
            TypeBody::AnyFloat | TypeBody::F32 | TypeBody::F64
        )
    }

    pub fn property<'a>(
        &'a self,
        name: &str,
        typedefs: &'a [TypeDef],
    ) -> Option<&'a Type> {
        match &self.body {
            TypeBody::Inferred(v) => v.properties.get(name),
            TypeBody::TypeRef(idx) => match &typedefs.get(*idx)?.body {
                TypeDefBody::Record(rec) => Some(&rec.fields.get(name)?.ty),
            },
            _ => None,
        }
    }

    pub fn intersection(&self, other: &Type, typedefs: &[TypeDef]) -> Option<Type> {
        let body = match (self, other) {
            number!(U8) => TypeBody::U8,
            number!(U16) => TypeBody::U16,
            number!(U32) => TypeBody::U32,
            number!(U64) => TypeBody::U64,
            number!(USize) => TypeBody::USize,
            number!(I8, AnySignedNumber) => TypeBody::I8,
            number!(I16, AnySignedNumber) => TypeBody::I16,
            number!(I32, AnySignedNumber) => TypeBody::I32,
            number!(I64, AnySignedNumber) => TypeBody::I64,
            number!(F32, AnySignedNumber, AnyFloat) => TypeBody::F32,
            number!(F64, AnySignedNumber, AnyFloat) => TypeBody::F64,
            (body!(TypeBody::String(a)), body!(TypeBody::String(b))) => {
                let len = match (&a.len, &b.len) {
                    (a_len, b_len) if a_len == b_len => a_len.clone(),
                    (Some(len), None) | (None, Some(len)) => Some(*len),
                    _ => return None,
                };
                TypeBody::String(StringType { len })
            }
            (body!(TypeBody::Array(a)), body!(TypeBody::Array(b))) => {
                let len = match (&a.len, &b.len) {
                    (a_len, b_len) if a_len == b_len => a_len.clone(),
                    (Some(len), None) | (None, Some(len)) => Some(*len),
                    _ => return None,
                };
                TypeBody::Array(ArrayType {
                    len,
                    item: a.item.intersection(&b.item, typedefs)?.into(),
                })
            }
            (body!(TypeBody::Inferred(a)), body!(TypeBody::Inferred(b))) => {
                let mut props = utils::SortedMap::new();
                let prop_names: HashSet<_> =
                    a.properties.keys().chain(b.properties.keys()).collect();
                for prop_name in prop_names {
                    let a_prop_ty = a.properties.get(prop_name);
                    let b_prop_ty = b.properties.get(prop_name);
                    let ty = match (a_prop_ty, b_prop_ty) {
                        (Some(a_prop), Some(b_prop)) => {
                            a_prop.intersection(b_prop, typedefs)?
                        }
                        (Some(prop), None) | (None, Some(prop)) => prop.clone(),
                        (None, None) => return None, // this should never happen
                    };
                    props.insert(prop_name.to_string(), ty);
                }
                TypeBody::Inferred(InferredType { properties: props })
            }
            unordered!(body!(TypeBody::Inferred(a)), b) => {
                let has_all_properties = a.properties.iter().all(|(name, a_ty)| {
                    other
                        .property(name, typedefs)
                        .is_some_and(|b_ty| a_ty.intersection(b_ty, typedefs).is_some())
                });
                if has_all_properties {
                    b.body.clone()
                } else {
                    return None;
                }
            }
            (body!(a), body!(b)) if a == b => a.clone(),
            _ => return None,
        };
        let loc = match (&self.loc, &other.loc) {
            unordered!(Some(loc), None) => Some(*loc),
            (Some(a), Some(b)) => {
                if a == b {
                    Some(*a)
                } else {
                    None
                }
            }
            (None, None) => None,
        };
        Some(Type::new(body, loc))
    }

    pub fn common_type(&self, other: &Type, typedefs: &[TypeDef]) -> Option<Type> {
        let body = match (self, other) {
            (body!(TypeBody::String(a)), body!(TypeBody::String(b))) => {
                TypeBody::String(StringType {
                    len: if a.len == b.len { a.len.clone() } else { None },
                })
            }
            (body!(TypeBody::Array(a)), body!(TypeBody::Array(b))) => {
                TypeBody::Array(ArrayType {
                    item: a.item.common_type(&b.item, typedefs)?.into(),
                    len: if a.len == b.len { a.len.clone() } else { None },
                })
            }
            (body!(TypeBody::Inferred(a)), body!(TypeBody::Inferred(b))) => {
                let mut props = utils::SortedMap::new();
                let prop_names: HashSet<_> =
                    a.properties.keys().chain(b.properties.keys()).collect();
                for prop_name in prop_names {
                    let a_prop_ty = a.properties.get(prop_name);
                    let b_prop_ty = b.properties.get(prop_name);
                    let ty = match (a_prop_ty, b_prop_ty) {
                        (Some(a_prop), Some(b_prop)) => {
                            a_prop.common_type(b_prop, typedefs)?
                        }
                        (Some(prop), None) | (None, Some(prop)) => prop.clone(),
                        (None, None) => return None, // this should never happen
                    };
                    props.insert(prop_name.to_string(), ty);
                }
                TypeBody::Inferred(InferredType { properties: props })
            }
            unordered!(body!(TypeBody::Inferred(a)), b) => {
                for (prop_name, prop_ty) in &a.properties {
                    if prop_ty
                        .common_type(other.property(prop_name, typedefs)?, typedefs)
                        .is_none()
                    {
                        return None;
                    }
                }
                return Some(b.clone());
            }
            (a, b) => {
                if &a.body == &b.body {
                    a.body.clone()
                } else {
                    return a.intersection(b, typedefs);
                }
            }
        };
        let loc = match (&self.loc, &other.loc) {
            unordered!(Some(loc), None) => Some(loc.clone()),
            (Some(_), Some(_)) | (None, None) => None,
        };
        Some(Type::new(body, loc))
    }
}
impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        &self.body == &other.body
    }
}
impl Eq for Type {}
impl Hash for Type {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.body.hash(state)
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        match &self.body {
            TypeBody::Bool => write!(f, "bool")?,
            TypeBody::AnyNumber => write!(f, "AnyNumber")?,
            TypeBody::AnySignedNumber => write!(f, "AnySignedNumber")?,
            TypeBody::AnyFloat => write!(f, "AnyFloat")?,
            TypeBody::I8 => write!(f, "i8")?,
            TypeBody::I16 => write!(f, "i16")?,
            TypeBody::I32 => write!(f, "i32")?,
            TypeBody::I64 => write!(f, "i64")?,
            TypeBody::U8 => write!(f, "u8")?,
            TypeBody::U16 => write!(f, "u16")?,
            TypeBody::U32 => write!(f, "u32")?,
            TypeBody::U64 => write!(f, "u64")?,
            TypeBody::USize => write!(f, "usize")?,
            TypeBody::F32 => write!(f, "f32")?,
            TypeBody::F64 => write!(f, "f64")?,
            TypeBody::Inferred(v) => {
                write!(f, "infered")?;
                for (name, t) in &v.properties {
                    write!(f, " .{}: {}", name, t)?;
                }
            }
            TypeBody::String(v) => {
                write!(f, "string")?;
                if let Some(len) = v.len {
                    write!(f, " {}", len)?;
                }
            }
            TypeBody::Array(v) => {
                write!(f, "array {}", v.item)?;
                if let Some(len) = v.len {
                    write!(f, " {}", len)?;
                }
            }
            TypeBody::TypeRef(i) => write!(f, "type {i}")?,
        }
        if let Some(loc) = &self.loc {
            write!(f, " {loc}")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InferredType {
    pub properties: utils::SortedMap<String, Type>,
}

impl InferredType {
    pub fn new(properties: impl IntoIterator<Item = (String, Type)>) -> Self {
        Self {
            properties: properties.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, new)]
pub struct StringType {
    pub len: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, new)]
pub struct ArrayType {
    pub item: Box<Type>,
    pub len: Option<usize>,
}
