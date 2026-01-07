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

#[derive(Error, Debug)]
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
            error: format!("{err:#?}"),
        })?;
        Ok(Self(result))
    }
}

pub struct Params<T>(pub T);
impl<S, T> FromRequest<S> for Params<T>
where
    T: DeserializeOwned,
{
    fn from_request(request: &Request<S>) -> RouterResult<Params<T>> {
        let parsed: T = serde_json::from_value(request.params.clone())?;
        Ok(Self(parsed))
    }
}

pub struct StateHandle<S>(pub S);
impl<S> FromRequest<S> for StateHandle<S>
where
    S: Clone + Send + Sync,
{
    fn from_request(request: &Request<S>) -> RouterResult<StateHandle<S>> {
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

    pub async fn route<'a, P, T, F>(
        &mut self,
        path: P,
        handler: F,
        qos: QoS,
        subcribe: bool,
    ) -> MqttResult
    where
        P: Into<String>,
        F: MakeDispatcher<T, S>,
    {
        let path = path.into();
        if subcribe {
            self.client.subscribe(&route_to_topic(&path), qos).await?;
        }
        let dispatcher = F::make_dispatcher(handler);
        self.router.insert(path, dispatcher)?;
        Ok(())
    }

    pub async fn dispatch(&self, message: Message, state: S) -> RouterResult {
        let topic = crate::bytes_to_string(&message.topic).ok_or(anyhow!("msg topic is empy"))?;

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

macro_rules! impl_make_dispatcher {
    (
        [$($ty:ident),*], $last:ident
    ) => {

impl<F, S, Fut, $($ty,)* $last> MakeDispatcher<($($ty,)* $last,), S> for F
where
    F: (Fn($($ty,)* $last) -> Fut) + Send + Sync + 'static,
    Fut: Future<Output = MqttResult> + Send ,
    S: Clone + Send + Sync + 'static,
    $( $ty: FromRequest<S>, )*
    $last: FromRequest<S>
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttResult> + Send>> + Send + Sync> =
            Box::new(move |request: Request<S>| {
                let func = func.clone();
                Box::pin(async move {
                    $(
                    let $ty = $ty::from_request(&request)?;
                    )*

                    let $last = $last::from_request(&request)?;

                    func($($ty,)* $last).await
                })
            });

        Dispatcher::new(wrap)
    }
}

    }
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

all_the_tuples!(impl_make_dispatcher);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_route_to_topic() {
        for (route, expected_topic) in [
            ("hello/{there}", "hello/+"),
            ("a/{b}/foo", "a/+/foo"),
            ("hello", "hello"),
        ] {
            let topic = route_to_topic(route);
            assert_eq!(
                topic, expected_topic,
                "route={route}, expected={expected_topic} actual={topic}"
            );
        }
    }

    #[test]
    fn routing() -> MqttResult {
        let mut router = Router::new();
        router.insert("pv2mqtt/home", "Welcome!")?;
        router.insert("pv2mqtt/users/{name}/{id}", "A User")?;
        let matched = router.at("pv2mqtt/users/foo/978")?;
        assert_eq!(matched.params.get("id"), Some("978"));
        assert_eq!(*matched.value, "A User");
        Ok(())
    }
}
