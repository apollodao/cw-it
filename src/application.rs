use cosmos_sdk_proto::cosmos::auth::v1beta1::BaseAccount;
use cosmrs::proto::cosmos::base::abci::v1beta1::GasInfo;
use cosmrs::{rpc::endpoint::abci_query::AbciQuery, tx::Fee, AccountId};
use prost::Message;
use test_tube::{RunnerResult, SigningAccount};
//use tendermint_rpc::endpoint::abci_query::AbciQuery;

pub trait Application {
    fn create_signed_tx<I>(
        &self,
        msgs: I,
        signer: &SigningAccount,
        fee: Fee,
    ) -> RunnerResult<Vec<u8>>
    where
        I: IntoIterator<Item = cosmrs::Any>;

    fn simulate_tx<I>(&self, msgs: I, signer: &SigningAccount) -> RunnerResult<GasInfo>
    where
        I: IntoIterator<Item = cosmrs::Any>;

    fn estimate_fee<I>(&self, msgs: I, signer: &SigningAccount) -> RunnerResult<Fee>
    where
        I: IntoIterator<Item = cosmrs::Any>;

    fn base_account(&self, account_id: AccountId) -> RunnerResult<BaseAccount>;
    fn abci_query<T: Message>(&self, req: T, path: &str) -> RunnerResult<AbciQuery>;
}
