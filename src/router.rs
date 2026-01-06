use crate::{MqttClient, QoS};
use anyhow::anyhow;
use matchit::Router;
pub use rumqttc::v5::mqttbytes::v5::Publish as Message;
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

// 添加路由结果
pub type RouterResult<T> = Result<T, RouterError>;
//调度器异步函数的 结果
pub type MqttHandlerResult = anyhow::Result<()>;

//调度器异步函数的入参
pub struct Request<S> {
    params: JsonValue,
    message: Message,
    state: S,
}

pub trait FromRequest<S>: Sized {
    fn from_request(request: &Request<S>) -> RouterResult<Self>;
}

// 将mqtt playload 转为:元组结构体
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

//将JSONVALUE 转为 元组结构体
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

//将调度器的状态 转为 元组结构体
pub struct StateHandle<S>(pub S);
impl<S> FromRequest<S> for StateHandle<S>
where
    S: Clone + Send + Sync,
{
    fn from_request(request: &Request<S>) -> RouterResult<StateHandle<S>> {
        Ok(Self(request.state.clone()))
    }
}

//调度器
pub struct Dispatcher<S = ()>
where
    S: Clone + Send + Sync,
{
    //调度器的异步函数
    func: Box<
        dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttHandlerResult> + Send>> + Send + Sync,
    >,
}

impl<S: Clone + Send + Sync + 'static> Dispatcher<S> {
    //调用异步函数
    pub async fn call(&self, params: JsonValue, message: Message, state: S) -> MqttHandlerResult {
        (self.func)(Request {
            params,
            message,
            state,
        })
        .await
    }

    pub fn new(
        func: Box<
            dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttHandlerResult> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { func }
    }
}

// 创建调度器
pub trait MakeDispatcher<T, S: Clone + Send + Sync> {
    fn make_dispatcher(func: Self) -> Dispatcher<S>;
}

//指定 泛型 S=（） 为单元类型
// 等价于显式声明： let router2: MqttRouter<()> = MqttRouter { ... };
pub struct MqttRouter<S = ()>
where
    S: Clone + Send + Sync,
{
    //路列表，
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

    // 添加路由
    // r.route(format!("{disco_prefix}/status"), mqtt_homeassitant_status).await?;
    pub async fn route<'a, P, T, F>(&mut self, path: P, handler: F, qos: QoS) -> RouterResult<()>
    where
        P: Into<String>,
        F: MakeDispatcher<T, S>,
    {
        let path = path.into();
        self.client.subscribe(&route_to_topic(&path), qos).await?;
        let dispatcher = F::make_dispatcher(handler);
        //向http 插入 异步函数值
        self.router.insert(path, dispatcher)?;
        Ok(())
    }

    // 调用路由
    pub async fn dispatch(&self, message: Message, state: S) -> RouterResult<()> {
        let topic = crate::bytes_to_string(&message.topic).ok_or(anyhow!("msg topic is empy"))?;
        //取值
        let matched = self
            .router
            .at(&topic)
            .map_err(|e| anyhow!("route :{} error:{}", topic, e))?;

        //获取url 的参数
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

//将 RUL 路由转 为 MQTT 主题
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
    Fut: Future<Output = MqttHandlerResult> + Send ,
    S: Clone + Send + Sync + 'static,
    $( $ty: FromRequest<S>, )*
    $last: FromRequest<S>
{
    #[allow(non_snake_case)]
    fn make_dispatcher(func: F) -> Dispatcher<S> {
        let func = Arc::new(func);
        let wrap: Box<dyn Fn(Request<S>) -> Pin<Box<dyn Future<Output = MqttHandlerResult> + Send>> + Send + Sync> =
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
    fn routing() -> RouterResult<()> {
        let mut router = Router::new();
        router.insert("pv2mqtt/home", "Welcome!")?;
        router.insert("pv2mqtt/users/{name}/{id}", "A User")?;
        let matched = router.at("pv2mqtt/users/foo/978")?;
        assert_eq!(matched.params.get("id"), Some("978"));
        assert_eq!(*matched.value, "A User");
        Ok(())
    }
}
