use crate::{Message, MqttClient, MqttResult, QoS};
use anyhow::anyhow;
use matchit::Router;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;

pub enum RouterError {
    #[error("payload is not utf8, cannot parse into string")]
    PayloadIsNotUtf8,
    #[error("failed to parse payload {text}: {error}")]
    PayloadParseFailed { text: String, error: String },
    #[error(transparent)]
    InsertError(#[from] matchit::InsertError),
    #[error(transparent)]
    MatchError(#[from] matchit::MatchError),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
    #[error(transparent)]
    Any(#[from] anyhow::Error),
}
#[allow(unused_qualifications)]
#[automatically_derived]
impl ::thiserror::__private17::Error for RouterError {
    fn source(&self) -> ::core::option::Option<&(dyn ::thiserror::__private17::Error + 'static)> {
        use ::thiserror::__private17::AsDynError as _;
        #[allow(deprecated)]
        match self {
            RouterError::PayloadIsNotUtf8 { .. } => ::core::option::Option::None,
            RouterError::PayloadParseFailed { .. } => ::core::option::Option::None,
            RouterError::InsertError { 0: transparent } => {
                ::thiserror::__private17::Error::source(transparent.as_dyn_error())
            }
            RouterError::MatchError { 0: transparent } => {
                ::thiserror::__private17::Error::source(transparent.as_dyn_error())
            }
            RouterError::JsonError { 0: transparent } => {
                ::thiserror::__private17::Error::source(transparent.as_dyn_error())
            }
            RouterError::Any { 0: transparent } => {
                ::thiserror::__private17::Error::source(transparent.as_dyn_error())
            }
        }
    }
}
#[allow(unused_qualifications)]
#[automatically_derived]
impl ::core::fmt::Display for RouterError {
    fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        use ::thiserror::__private17::AsDisplay as _;
        #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
        match self {
            RouterError::PayloadIsNotUtf8 {} => {
                __formatter.write_str("payload is not utf8, cannot parse into string")
            }
            RouterError::PayloadParseFailed { text, error } => {
                match (text.as_display(), error.as_display()) {
                    (__display_text, __display_error) => __formatter.write_fmt(format_args!(
                        "failed to parse payload {0}: {1}",
                        __display_text, __display_error,
                    )),
                }
            }
            RouterError::InsertError(_0) => ::core::fmt::Display::fmt(_0, __formatter),
            RouterError::MatchError(_0) => ::core::fmt::Display::fmt(_0, __formatter),
            RouterError::JsonError(_0) => ::core::fmt::Display::fmt(_0, __formatter),
            RouterError::Any(_0) => ::core::fmt::Display::fmt(_0, __formatter),
        }
    }
}
#[allow(
    deprecated,
    unused_qualifications,
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes
)]
#[automatically_derived]
impl ::core::convert::From<matchit::InsertError> for RouterError {
    fn from(source: matchit::InsertError) -> Self {
        RouterError::InsertError { 0: source }
    }
}
#[allow(
    deprecated,
    unused_qualifications,
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes
)]
#[automatically_derived]
impl ::core::convert::From<matchit::MatchError> for RouterError {
    fn from(source: matchit::MatchError) -> Self {
        RouterError::MatchError { 0: source }
    }
}
#[allow(
    deprecated,
    unused_qualifications,
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes
)]
#[automatically_derived]
impl ::core::convert::From<serde_json::Error> for RouterError {
    fn from(source: serde_json::Error) -> Self {
        RouterError::JsonError { 0: source }
    }
}
#[allow(
    deprecated,
    unused_qualifications,
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes
)]
#[automatically_derived]
impl ::core::convert::From<anyhow::Error> for RouterError {
    fn from(source: anyhow::Error) -> Self {
        RouterError::Any { 0: source }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for RouterError {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match self {
            RouterError::PayloadIsNotUtf8 => {
                ::core::fmt::Formatter::write_str(f, "PayloadIsNotUtf8")
            }
            RouterError::PayloadParseFailed {
                text: __self_0,
                error: __self_1,
            } => ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "PayloadParseFailed",
                "text",
                __self_0,
                "error",
                &__self_1,
            ),
            RouterError::InsertError(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(f, "InsertError", &__self_0)
            }
            RouterError::MatchError(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(f, "MatchError", &__self_0)
            }
            RouterError::JsonError(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(f, "JsonError", &__self_0)
            }
            RouterError::Any(__self_0) => {
                ::core::fmt::Formatter::debug_tuple_field1_finish(f, "Any", &__self_0)
            }
        }
    }
}
pub type RouterResult<T = ()> = Result<T, RouterError>;
pub struct Request<S> {
    params: JsonValue,
    message: Message,
    state: S,
}
pub trait FromRequest<S>: Sized {
    fn from_request(request: &Request<S>) -> RouterResult<Self>;
}
pub struct Payload<T>(pub T);
impl<S, T> FromRequest<S> for Payload<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Debug,
{
    fn from_request(request: &Request<S>) -> RouterResult<Payload<T>> {
        let s = std::str::from_utf8(&request.message.payload)
            .map_err(|_| RouterError::PayloadIsNotUtf8)?;
        let result: T = s.parse().map_err(|err| RouterError::PayloadParseFailed {
            text: s.to_string(),
            error: ::alloc::__export::must_use({
                ::alloc::fmt::format(format_args!("{0:#?}", err))
            }),
        })?;
        Ok(Self(result))
    }
}
pub struct Params<T>(pub T);
impl<S, T> FromRequest<S> for Params<T>
where
    T: DeserializeOwned,
{
    fn from_request(request: &Request<S>) -> RouterResult<Self> {
        let parsed: T = serde_json::from_value(request.params.clone())?;
        Ok(Self(parsed))
    }
}
pub struct StateHandle<S>(pub S);
impl<S> FromRequest<S> for StateHandle<S>
where
    S: Clone + Send + Sync,
{
    fn from_request(request: &Request<S>) -> RouterResult<Self> {
        Ok(Self(request.state.clone()))
    }
}
pub struct Dispatcher<S = ()>
where
    S: Clone + Send + Sync,
{
    func: Box<dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync>,
}
impl<S: Clone + Send + Sync + 'static> Dispatcher<S> {
    pub async fn call(&self, params: JsonValue, message: Message, state: S) -> MqttResult {
        (self.func)(Request {
            params,
            message,
            state,
        })
        .await
    }
    pub fn new(
        func: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        >,
    ) -> Self {
        Self { func }
    }
}
pub trait MakeDispatcher<T, S: Clone + Send + Sync> {
    fn make_dispatcher(func: Self) -> Dispatcher<S>;
}
pub struct MqttRouter<S = ()>
where
    S: Clone + Send + Sync,
{
    router: Router<Dispatcher<S>>,
    client: MqttClient,
}
impl<S: Clone + Send + Sync + 'static> MqttRouter<S> {
    pub fn new(client: MqttClient) -> Self {
        Self {
            router: Router::new(),
            client,
        }
    }
    pub async fn subscribe<'a, P, T, F>(&mut self, path: P, handler: F, qos: QoS) -> MqttResult
    where
        P: Into<String>,
        F: MakeDispatcher<T, S>,
    {
        let path = path.into();
        self.client.subscribe(&route_to_topic(&path), qos).await?;
        let dispatcher = F::make_dispatcher(handler);
        self.router.insert(path, dispatcher)?;
        Ok(())
    }
    pub fn add<'a, P, T, F>(&mut self, path: P, handler: F) -> MqttResult
    where
        P: Into<String>,
        F: MakeDispatcher<T, S>,
    {
        let path = path.into();
        let dispatcher = F::make_dispatcher(handler);
        self.router.insert(path, dispatcher)?;
        Ok(())
    }
    pub fn remove<'a, P, T, F>(&mut self, path: P) -> MqttResult
    where
        P: Into<String>,
    {
        let path = path.into();
        self.router.remove(path);
        Ok(())
    }
    pub async fn dispatch(&self, message: Message, state: S) -> RouterResult {
        let topic =
            crate::bytes_to_string(&message.topic).ok_or(::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(format_args!("msg topic is empy"));
                error
            }))?;
        let matched = self.router.at(&topic)?;
        let params = {
            let mut value_map = serde_json::Map::new();
            for (k, v) in matched.params.iter() {
                value_map.insert(k.into(), v.into());
            }
            if value_map.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Object(value_map)
            }
        };
        let v = matched.value.call(params, message, state).await?;
        Ok(v)
    }
}
fn route_to_topic(route: &str) -> String {
    let mut result = String::new();
    let mut in_param = false;
    for c in route.chars() {
        match c {
            '{' => {
                in_param = true;
                result.push('+');
            }
            '}' => {
                in_param = false;
            }
            '/' if !in_param => {
                result.push('/');
            }
            _ if !in_param => {
                result.push(c);
            }
            _ => {}
        }
    }
    result
}

impl<F, S, Fut, T1> MakeDispatcher<(T1,), S> for F
where
    F: Fn(T1) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                func(T1).await
            })
        });
        Dispatcher::new(wrap)
    }
}

impl<F, S, Fut, T1, T2> MakeDispatcher<(T1, T2), S> for F
where
    F: Fn(T1, T2) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                func(T1, T2).await
            })
        });
        Dispatcher::new(wrap)
    }
}

impl<F, S, Fut, T1, T2, T3> MakeDispatcher<(T1, T2, T3), S> for F
where
    F: Fn(T1, T2, T3) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                func(T1, T2, T3).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4> MakeDispatcher<(T1, T2, T3, T4), S> for F
where
    F: Fn(T1, T2, T3, T4) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                func(T1, T2, T3, T4).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5> MakeDispatcher<(T1, T2, T3, T4, T5), S> for F
where
    F: Fn(T1, T2, T3, T4, T5) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                func(T1, T2, T3, T4, T5).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6> MakeDispatcher<(T1, T2, T3, T4, T5, T6), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7> MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8> MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8), S>
    for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
    T12: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                let T12 = T12::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
    T12: FromRequest<S>,
    T13: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                let T12 = T12::from_request(&request)?;
                let T13 = T13::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14>
    MakeDispatcher<(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14), S> for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
    T12: FromRequest<S>,
    T13: FromRequest<S>,
    T14: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                let T12 = T12::from_request(&request)?;
                let T13 = T13::from_request(&request)?;
                let T14 = T14::from_request(&request)?;
                func(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14).await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15>
    MakeDispatcher<
        (
            T1,
            T2,
            T3,
            T4,
            T5,
            T6,
            T7,
            T8,
            T9,
            T10,
            T11,
            T12,
            T13,
            T14,
            T15,
        ),
        S,
    > for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
    T12: FromRequest<S>,
    T13: FromRequest<S>,
    T14: FromRequest<S>,
    T15: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                let T12 = T12::from_request(&request)?;
                let T13 = T13::from_request(&request)?;
                let T14 = T14::from_request(&request)?;
                let T15 = T15::from_request(&request)?;
                func(
                    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15,
                )
                .await
            })
        });
        Dispatcher::new(wrap)
    }
}
impl<F, S, Fut, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16>
    MakeDispatcher<
        (
            T1,
            T2,
            T3,
            T4,
            T5,
            T6,
            T7,
            T8,
            T9,
            T10,
            T11,
            T12,
            T13,
            T14,
            T15,
            T16,
        ),
        S,
    > for F
where
    F: Fn(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16) -> Fut
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = MqttResult> + Send,
    S: Clone + Send + Sync + 'static,
    T1: FromRequest<S>,
    T2: FromRequest<S>,
    T3: FromRequest<S>,
    T4: FromRequest<S>,
    T5: FromRequest<S>,
    T6: FromRequest<S>,
    T7: FromRequest<S>,
    T8: FromRequest<S>,
    T9: FromRequest<S>,
    T10: FromRequest<S>,
    T11: FromRequest<S>,
    T12: FromRequest<S>,
    T13: FromRequest<S>,
    T14: FromRequest<S>,
    T15: FromRequest<S>,
    T16: FromRequest<S>,
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync,
        > = Box::new(move |request: Request<S>| {
            let func = func.clone();
            Box::pin(async move {
                let T1 = T1::from_request(&request)?;
                let T2 = T2::from_request(&request)?;
                let T3 = T3::from_request(&request)?;
                let T4 = T4::from_request(&request)?;
                let T5 = T5::from_request(&request)?;
                let T6 = T6::from_request(&request)?;
                let T7 = T7::from_request(&request)?;
                let T8 = T8::from_request(&request)?;
                let T9 = T9::from_request(&request)?;
                let T10 = T10::from_request(&request)?;
                let T11 = T11::from_request(&request)?;
                let T12 = T12::from_request(&request)?;
                let T13 = T13::from_request(&request)?;
                let T14 = T14::from_request(&request)?;
                let T15 = T15::from_request(&request)?;
                let T16 = T16::from_request(&request)?;
                func(
                    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16,
                )
                .await
            })
        });
        Dispatcher::new(wrap)
    }
}
