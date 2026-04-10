use super::*;
use domain::transaction::*;

#[fixture]
async fn app(_setup: (), ctx: &'static Context) -> Router {
    let svc = Arc::new(ctx.setup::<_, KafkaSender<TransactionCommand>>().await);
    Router::new().merge(routes::transaction_routes().with_state(svc))
}

prop_compose! {
    fn valid_init() (
        account_code in digit_string(6),
        limit in 10_000..10_000_000i64
    ) -> InitPeriod {
        InitPeriod { account_code, limit }
    }
}

prop_compose! {
    fn valid_open() (
        account_code in digit_string(6)
    ) -> OpenPeriod {
        OpenPeriod { account_code }
    }
}

prop_compose! {
    fn valid_set_limit() (
        limit in 10_000..10_000_000i64,
        trans_limit in 10_000..10_000_000i64,
        balance in 0..i64::MAX
    ) -> SetLimit {
        SetLimit { limit, trans_limit, balance }
    }
}

prop_compose! {
    fn valid_change_limit() (
        limit in 10_000..10_000_000i64
    ) -> ChangeLimit {
        ChangeLimit { limit }
    }
}

prop_compose! {
    fn valid_set_trans_limit() (
        trans_limit in 10_000..10_000_000i64
    ) -> SetTransLimit {
        SetTransLimit { trans_limit }
    }
}

prop_compose! {
    fn valid_deposit() (
        amount in 1..i64::MAX
    ) -> Deposit {
        Deposit { amount }
    }
}

prop_compose! {
    fn valid_withdraw() (
        amount in 1..i64::MAX
    ) -> Withdraw {
        Withdraw { amount }
    }
}

prop_compose! {
    fn valid_transfer_in() (
        out_code in digit_string(6),
        amount in 1..i64::MAX
    ) -> TransferIn {
        TransferIn { out_code, amount }
    }
}

prop_compose! {
    fn valid_transfer_out() (
        in_code in digit_string(6),
        amount in 1..i64::MAX
    ) -> TransferOut {
        TransferOut { in_code, amount }
    }
}
