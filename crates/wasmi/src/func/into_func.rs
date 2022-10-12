use super::{
    super::engine::{FuncParams, FuncResults},
    HostFuncTrampoline,
};
use crate::{
    core::{FromValue, Trap, Value, ValueType, F32, F64},
    foreach_tuple::for_each_tuple,
    Caller,
    FuncType,
};
use core::{array, iter::FusedIterator};
use wasmi_core::{DecodeUntypedSlice, EncodeUntypedSlice, UntypedValue};

/// Closures and functions that can be used as host functions.
pub trait IntoFunc<T, Params, Results>: Send + Sync + 'static {
    /// The parameters of the host function.
    #[doc(hidden)]
    type Params: WasmTypeList;
    /// The results of the host function.
    #[doc(hidden)]
    type Results: WasmTypeList;

    /// Converts the function into its `wasmi` signature and its trampoline.
    #[doc(hidden)]
    fn into_func(self) -> (FuncType, HostFuncTrampoline<T>);
}

macro_rules! impl_into_func {
    ( $n:literal $( $tuple:ident )* ) => {
        impl<T, F, $($tuple,)* R> IntoFunc<T, ($($tuple,)*), R> for F
        where
            F: Fn($($tuple),*) -> R,
            F: Send + Sync + 'static,
            $(
                $tuple: WasmType,
            )*
            R: WasmRet,
        {
            type Params = ($($tuple,)*);
            type Results = <R as WasmRet>::Ok;

            #[allow(non_snake_case)]
            fn into_func(self) -> (FuncType, HostFuncTrampoline<T>) {
                IntoFunc::into_func(
                    move |
                        _: Caller<'_, T>,
                        $(
                            $tuple: $tuple,
                        )*
                    | {
                        (self)($($tuple),*)
                    }
                )
            }
        }

        impl<T, F, $($tuple,)* R> IntoFunc<T, (Caller<'_, T>, $($tuple),*), R> for F
        where
            F: Fn(Caller<T>, $($tuple),*) -> R,
            F: Send + Sync + 'static,
            $(
                $tuple: WasmType,
            )*
            R: WasmRet,
        {
            type Params = ($($tuple,)*);
            type Results = <R as WasmRet>::Ok;

            #[allow(non_snake_case)]
            fn into_func(self) -> (FuncType, HostFuncTrampoline<T>) {
                let signature = FuncType::new(
                    <Self::Params as WasmTypeList>::value_types(),
                    <Self::Results as WasmTypeList>::value_types(),
                );
                let trampoline = HostFuncTrampoline::new(
                    move |caller: Caller<T>, params_results: FuncParams| -> Result<FuncResults, Trap> {
                        let ($($tuple,)*): Self::Params = params_results.read_params();
                        let results: Self::Results =
                            (self)(caller, $($tuple),*).into_fallible()?;
                        Ok(params_results.write_results(results))
                    },
                );
                (signature, trampoline)
            }
        }
    };
}
for_each_tuple!(impl_into_func);

/// Types and type sequences that can be used as return values of host functions.
pub trait WasmRet {
    #[doc(hidden)]
    type Ok: WasmTypeList;

    #[doc(hidden)]
    fn into_fallible(self) -> Result<<Self as WasmRet>::Ok, Trap>;
}

impl<T1> WasmRet for T1
where
    T1: WasmType,
{
    type Ok = T1;

    #[inline]
    fn into_fallible(self) -> Result<Self::Ok, Trap> {
        Ok(self)
    }
}

impl<T1> WasmRet for Result<T1, Trap>
where
    T1: WasmType,
{
    type Ok = T1;

    #[inline]
    fn into_fallible(self) -> Result<<Self as WasmRet>::Ok, Trap> {
        self
    }
}

macro_rules! impl_wasm_return_type {
    ( $n:literal $( $tuple:ident )* ) => {
        impl<$($tuple),*> WasmRet for ($($tuple,)*)
        where
            $(
                $tuple: WasmType
            ),*
        {
            type Ok = ($($tuple,)*);

            #[inline]
            fn into_fallible(self) -> Result<Self::Ok, Trap> {
                Ok(self)
            }
        }

        impl<$($tuple),*> WasmRet for Result<($($tuple,)*), Trap>
        where
            $(
                $tuple: WasmType
            ),*
        {
            type Ok = ($($tuple,)*);

            #[inline]
            fn into_fallible(self) -> Result<<Self as WasmRet>::Ok, Trap> {
                self
            }
        }
    };
}
for_each_tuple!(impl_wasm_return_type);

/// Types that can be used as parameters or results of host functions.
pub trait WasmType: FromValue + Into<Value> + From<UntypedValue> + Into<UntypedValue> {
    /// Returns the value type of the Wasm type.
    fn value_type() -> ValueType;
}

macro_rules! impl_wasm_type {
    ( $( type $rust_type:ty = $wasmi_type:ident );* $(;)? ) => {
        $(
            impl WasmType for $rust_type {
                #[inline]
                fn value_type() -> ValueType {
                    ValueType::$wasmi_type
                }
            }
        )*
    };
}
impl_wasm_type! {
    type u32 = I32;
    type u64 = I64;
    type i32 = I32;
    type i64 = I64;
    type F32 = F32;
    type F64 = F64;
}

/// A list of [`WasmType`] types.
///
/// # Note
///
/// This is a convenience trait that allows to:
///
/// - Read host function parameters from a region of the value stack.
/// - Write host function results into a region of the value stack.
/// - Iterate over the value types of the Wasm type sequence
///     - This is useful to construct host function signatures.
pub trait WasmTypeList: DecodeUntypedSlice + EncodeUntypedSlice + Sized {
    /// The number of Wasm types in the list.
    const LEN: usize;

    /// The [`ValueType`] sequence as array.
    type Types: IntoIterator<IntoIter = Self::TypesIter, Item = ValueType>
        + AsRef<[ValueType]>
        + AsMut<[ValueType]>;

    /// The iterator type of the sequence of [`ValueType`].
    type TypesIter: ExactSizeIterator<Item = ValueType> + DoubleEndedIterator + FusedIterator;

    /// The [`Value`] sequence as array.
    type Values: IntoIterator<IntoIter = Self::ValuesIter, Item = Value>
        + AsRef<[Value]>
        + AsMut<[Value]>;

    /// The iterator type of the sequence of [`Value`].
    type ValuesIter: ExactSizeIterator<Item = Value> + DoubleEndedIterator + FusedIterator;

    /// Returns an array representing the [`ValueType`] sequence of `Self`.
    fn value_types() -> Self::Types;

    /// Returns an array representing the [`Value`] sequence of `self`.
    fn values(self) -> Self::Values;

    /// Consumes the [`Value`] iterator and creates `Self` if possible.
    ///
    /// Returns `None` if construction of `Self` is impossible.
    fn from_values<T>(values: T) -> Option<Self>
    where
        T: Iterator<Item = Value>;
}

impl<T1> WasmTypeList for T1
where
    T1: WasmType,
{
    const LEN: usize = 1;

    type Types = [ValueType; 1];
    type TypesIter = array::IntoIter<ValueType, 1>;
    type Values = [Value; 1];
    type ValuesIter = array::IntoIter<Value, 1>;

    #[inline]
    fn value_types() -> Self::Types {
        [<T1 as WasmType>::value_type()]
    }

    #[inline]
    fn values(self) -> Self::Values {
        [<T1 as Into<Value>>::into(self)]
    }

    fn from_values<T>(mut values: T) -> Option<Self>
    where
        T: Iterator<Item = Value>,
    {
        let value: T1 = values.next().and_then(Value::try_into)?;
        if values.next().is_some() {
            // Note: If the iterator yielded more items than
            //       necessary we create no value from this procedure
            //       as it is likely a bug.
            return None;
        }
        Some(value)
    }
}

macro_rules! impl_wasm_type_list {
    ( $n:literal $( $tuple:ident )* ) => {
        impl<$($tuple),*> WasmTypeList for ($($tuple,)*)
        where
            $(
                $tuple: WasmType
            ),*
        {
            const LEN: usize = $n;

            type Types = [ValueType; $n];
            type TypesIter = array::IntoIter<ValueType, $n>;
            type Values = [Value; $n];
            type ValuesIter = array::IntoIter<Value, $n>;

            fn value_types() -> Self::Types {
                [$(
                    <$tuple as WasmType>::value_type()
                ),*]
            }

            #[allow(non_snake_case)]
            fn values(self) -> Self::Values {
                let ($($tuple,)*) = self;
                [$(
                    <$tuple as Into<Value>>::into($tuple)
                ),*]
            }

            fn from_values<T>(mut values: T) -> Option<Self>
            where
                T: Iterator<Item = Value>,
            {
                let result = ($(
                    values.next().and_then(Value::try_into::<$tuple>)?,
                )*);
                if values.next().is_some() {
                    // Note: If the iterator yielded more items than
                    //       necessary we create no value from this procedure
                    //       as it is likely a bug.
                    return None
                }
                Some(result)
            }
        }
    };
}
for_each_tuple!(impl_wasm_type_list);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{F32, F64};

    /// Utility struct helper for the `implements_wasm_results` macro.
    pub struct ImplementsWasmRet<T> {
        marker: core::marker::PhantomData<fn() -> T>,
    }
    /// Utility trait for the fallback case of the `implements_wasm_results` macro.
    pub trait ImplementsWasmRetFallback {
        const VALUE: bool = false;
    }
    impl<T> ImplementsWasmRetFallback for ImplementsWasmRet<T> {}
    /// Utility trait impl for the `true` case of the `implements_wasm_results` macro.
    impl<T> ImplementsWasmRet<T>
    where
        T: WasmRet,
    {
        // We need to allow for dead code at this point because
        // the Rust compiler thinks this function is unused even
        // though it acts as the specialized case for detection.
        #[allow(dead_code)]
        pub const VALUE: bool = true;
    }
    /// Returns `true` if the given type `T` implements the `WasmRet` trait.
    #[macro_export]
    #[doc(hidden)]
    macro_rules! implements_wasm_results {
        ( $T:ty $(,)? ) => {{
            #[allow(unused_imports)]
            use ImplementsWasmRetFallback as _;
            ImplementsWasmRet::<$T>::VALUE
        }};
    }

    #[test]
    fn into_func_trait_impls() {
        assert!(implements_wasm_results!(()));
        assert!(implements_wasm_results!(i32));
        assert!(implements_wasm_results!((i32,)));
        assert!(implements_wasm_results!((i32, u32, i64, u64, F32, F64)));
        assert!(implements_wasm_results!(Result<(), Trap>));
        assert!(implements_wasm_results!(Result<i32, Trap>));
        assert!(implements_wasm_results!(Result<(i32,), Trap>));
        assert!(implements_wasm_results!(Result<(i32, u32, i64, u64, F32, F64), Trap>));
    }
}
