/// 为聚合类型构造路由
#[macro_export]
macro_rules! route_builder {
    ($agg:ident, $format:ty, [$($cr:ident), *], [$($ch:ident), *]) => {{
        let mut router = Router::new();
        $(
            router = router.route(
                concat!("/", stringify!($agg), "/", stringify!($cr), "/{com_id}"),
                post($cr::<$format>),
            );
        )*
        $(
            router = router.route(
                concat!("/", stringify!($agg), "/", stringify!($ch), "/{agg_id}/{com_id}"),
                post($ch::<$format>),
            );
        )*
        router
    }};
}

/// 为创建聚合的命令构造处理器
#[macro_export]
macro_rules! create_handler {
    ($func_name:ident, $c:ty, $com:ty, $variant:ident) => {
        pub async fn $func_name<F>(
            Path(com_id): Path<Uuid>,
            State(svc): State<Arc<Sender<$c>>>,
            UniCommand(com, _): UniCommand<$com, F>,
        ) -> UniResponse {
            svc.create(com_id, <$c>::$variant(com)).await
        }
    };
}

/// 为变更聚合的命令构造处理器
#[macro_export]
macro_rules! change_handler {
    ($func_name:ident, $c:ty, $com:ty, $variant:ident) => {
        pub async fn $func_name<F>(
            Path(UniKey { agg_id, com_id }): Path<UniKey>,
            State(svc): State<Arc<Sender<$c>>>,
            UniCommand(com, _): UniCommand<$com, F>,
        ) -> UniResponse {
            svc.change(agg_id, com_id, <$c>::$variant(com)).await
        }
    };
}
