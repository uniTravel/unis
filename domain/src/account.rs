mod approve;
mod create;
mod limit;
mod verify;

pub use approve::ApproveAccount;
pub use create::CreateAccount;
pub use limit::LimitAccount;
pub use verify::VerifyAccount;

use unis::{
    domain::{Command, CommandEnum, Event, Load},
    errors::UniError,
    macros::{aggregate, command_enum, event_enum},
};
use uuid::Uuid;

#[aggregate]
pub struct Account {
    code: String,
    owner: String,
    limit: i64,
    verified_by: String,
    verified: bool,
    verify_conclusion: bool,
    approved_by: String,
    approved: bool,
}

#[event_enum(Account)]
pub enum AccountEvent {
    Created(create::AccountCreated) = 0,
    Verified(verify::AccountVerified) = 1,
    Limited(limit::AccountLimited) = 2,
    Approved(approve::AccountApproved) = 3,
}

#[command_enum]
pub enum AccountCommand {
    Create(create::CreateAccount) = 0,
    Verify(verify::VerifyAccount) = 1,
    Limit(limit::LimitAccount) = 2,
    Approve(approve::ApproveAccount) = 3,
}

impl CommandEnum for AccountCommand {
    type A = Account;
    type E = AccountEvent;

    async fn apply(
        self,
        topic: &'static str,
        agg_id: Uuid,
        mut agg: Self::A,
        coms: &mut ahash::AHashSet<Uuid>,
        loader: impl Load<Self::E>,
    ) -> Result<(Self::A, Self::E), UniError> {
        match self {
            AccountCommand::Create(com) => {
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Created(evt)))
            }
            AccountCommand::Verify(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Verified(evt)))
            }
            AccountCommand::Limit(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Limited(evt)))
            }
            AccountCommand::Approve(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Approved(evt)))
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use proptest::prelude::*;
//     use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};
//     use unis::domain::{Aggregate, Command};

//     #[derive(Debug, Clone)]
//     pub struct RefAccount {
//         code: String,
//         limit: i64,
//         verified: bool,
//         verify_conclusion: bool,
//         approved: bool,
//     }

//     impl ReferenceStateMachine for RefAccount {
//         type State = RefAccount;
//         type Transition = AccountCommand;

//         fn init_state() -> BoxedStrategy<Self::State> {
//             Just(RefAccount {
//                 code: String::default(),
//                 limit: 0,
//                 verified: false,
//                 verify_conclusion: false,
//                 approved: false,
//             })
//             .boxed()
//         }

//         fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
//             match state {
//                 RefAccount {
//                     code,
//                     limit: 0,
//                     verified: false,
//                     verify_conclusion: false,
//                     approved: false,
//                 } if code == &String::default() => {
//                     prop_oneof![create::tests::valid_com().prop_map(AccountCommand::Create)].boxed()
//                 }
//                 RefAccount {
//                     code: _,
//                     limit: 0,
//                     verified: false,
//                     verify_conclusion: false,
//                     approved: false,
//                 } => {
//                     prop_oneof![verify::tests::valid_com().prop_map(AccountCommand::Verify)].boxed()
//                 }
//                 // RefAccount {
//                 //     code: _,
//                 //     limit: 0,
//                 //     verified: true,
//                 //     verify_conclusion: false,
//                 //     approved: false,
//                 // } => prop_oneof![].boxed(),
//                 RefAccount {
//                     code: _,
//                     limit: 0,
//                     verified: true,
//                     verify_conclusion: true,
//                     approved: false,
//                 } => prop_oneof![limit::tests::valid_com().prop_map(AccountCommand::Limit)].boxed(),
//                 _ => panic!("非法状态"),
//             }
//         }

//         fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
//             match transition {
//                 AccountCommand::Create(com) => {
//                     state.code = com.code.clone();
//                     state
//                 }
//                 AccountCommand::Verify(com) => {
//                     state.verified = true;
//                     state.verify_conclusion = com.conclusion;
//                     state
//                 }
//                 AccountCommand::Limit(com) => {
//                     state.limit = com.limit;
//                     state
//                 }
//                 AccountCommand::Approve(com) => {
//                     state.approved = com.approved;
//                     state
//                 }
//             }
//         }
//     }

//     impl StateMachineTest for Account {
//         type SystemUnderTest = Account;
//         type Reference = RefAccount;

//         fn init_test(
//             _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
//         ) -> Self::SystemUnderTest {
//             Account::new(uuid::Uuid::new_v4())
//         }

//         fn apply(
//             mut agg: Self::SystemUnderTest,
//             _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
//             transition: <Self::Reference as ReferenceStateMachine>::Transition,
//         ) -> Self::SystemUnderTest {
//             match transition {
//                 AccountCommand::Create(com) => {
//                     let _ = com.process(&mut agg);
//                     agg
//                 }
//                 AccountCommand::Verify(com) => {
//                     let _ = com.process(&mut agg);
//                     agg
//                 }
//                 AccountCommand::Limit(com) => {
//                     let _ = com.process(&mut agg);
//                     agg
//                 }
//                 AccountCommand::Approve(com) => {
//                     let _ = com.process(&mut agg);
//                     agg
//                 }
//             }
//         }
//     }

//     prop_state_machine! {
//         #[test]
//         fn account_state_machine(sequential 1..10 => Account);
//     }
// }
