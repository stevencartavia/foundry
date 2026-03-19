use crate::{
    eth::backend::{cheats::CheatsManager, env::Env},
    mem::inspector::AnvilInspector,
};
use alloy_consensus::{Eip658Value, Transaction, TransactionEnvelope, transaction::Either};
use alloy_eips::{
    Encodable2718, eip2935, eip4788,
    eip7702::{RecoveredAuthority, RecoveredAuthorization},
};
use alloy_evm::{
    EthEvmFactory, Evm, EvmEnv, EvmFactory, FromRecoveredTx, FromTxWithEncoded, RecoveredTx,
    block::{
        BlockExecutionError, BlockExecutionResult, BlockExecutor, BlockValidationError,
        ExecutableTx, OnStateHook, StateChangeSource, StateDB, TxResult,
    },
    eth::{EthEvmContext, EthTxResult},
    precompiles::PrecompilesMap,
};
use alloy_op_evm::OpEvmFactory;
use alloy_primitives::{Address, B256, Bytes};
use anvil_core::eth::transaction::PendingTransaction;
use foundry_evm::{backend::DatabaseError, core::either_evm::EitherEvm};
use foundry_evm_networks::NetworkConfigs;
use foundry_primitives::{FoundryReceiptEnvelope, FoundryTxEnvelope, FoundryTxType};
use op_revm::{OpContext, OpTransaction};
use revm::{
    Database, DatabaseCommit, Inspector,
    context::{Block as RevmBlock, TxEnv},
    context_interface::result::{ExecutionResult, ResultAndState},
};
use std::{fmt, fmt::Debug};

/// Extended receipt-building context that carries `sender`.
///
/// Mirrors [`alloy_evm::eth::receipt_builder::ReceiptBuilderCtx`] with an
/// additional `sender` field needed for deposit nonce resolution.
#[derive(Debug)]
pub struct AnvilReceiptBuilderCtx<'a, T, E: Evm> {
    pub tx_type: T,
    pub sender: Address,
    pub evm: &'a E,
    pub result: ExecutionResult<E::HaltReason>,
    pub state: &'a revm::state::EvmState,
    pub cumulative_gas_used: u64,
}

/// Receipt builder for Anvil block execution.
///
/// Mirrors [`alloy_evm::eth::receipt_builder::ReceiptBuilder`] but uses
/// [`AnvilReceiptBuilderCtx`] which carries `sender: Address`.
pub trait AnvilReceiptBuilder: fmt::Debug + Send + Sync + 'static {
    type Transaction: TransactionEnvelope + Encodable2718;
    type Receipt: alloy_consensus::TxReceipt<Log = alloy_primitives::Log> + Clone + fmt::Debug;

    fn build_receipt<E: Evm>(
        &self,
        ctx: AnvilReceiptBuilderCtx<'_, <Self::Transaction as TransactionEnvelope>::TxType, E>,
    ) -> Self::Receipt;
}

/// Receipt builder for Foundry that handles all [`FoundryTxType`] variants.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct FoundryReceiptBuilder;

impl AnvilReceiptBuilder for FoundryReceiptBuilder {
    type Transaction = FoundryTxEnvelope;
    type Receipt = FoundryReceiptEnvelope;

    fn build_receipt<E: Evm>(
        &self,
        ctx: AnvilReceiptBuilderCtx<'_, FoundryTxType, E>,
    ) -> FoundryReceiptEnvelope {
        let receipt = alloy_consensus::Receipt {
            status: Eip658Value::Eip658(ctx.result.is_success()),
            cumulative_gas_used: ctx.cumulative_gas_used,
            logs: ctx.result.into_logs(),
        }
        .with_bloom();

        match ctx.tx_type {
            FoundryTxType::Legacy => FoundryReceiptEnvelope::Legacy(receipt),
            FoundryTxType::Eip2930 => FoundryReceiptEnvelope::Eip2930(receipt),
            FoundryTxType::Eip1559 => FoundryReceiptEnvelope::Eip1559(receipt),
            FoundryTxType::Eip4844 => FoundryReceiptEnvelope::Eip4844(receipt),
            FoundryTxType::Eip7702 => FoundryReceiptEnvelope::Eip7702(receipt),
            FoundryTxType::Tempo => FoundryReceiptEnvelope::Tempo(receipt),
            FoundryTxType::Deposit => {
                let deposit_nonce = ctx.state.get(&ctx.sender).map(|acc| acc.info.nonce);
                FoundryReceiptEnvelope::Deposit(op_alloy_consensus::OpDepositReceiptWithBloom {
                    receipt: op_alloy_consensus::OpDepositReceipt {
                        inner: receipt.receipt,
                        deposit_nonce,
                        deposit_receipt_version: deposit_nonce.map(|_| 1),
                    },
                    logs_bloom: receipt.logs_bloom,
                })
            }
        }
    }
}

/// Result of executing a transaction in [`AnvilBlockExecutor`].
///
/// Wraps [`EthTxResult`] with the sender address, needed for deposit nonce resolution.
#[derive(Debug)]
pub struct AnvilTxResult<H, T = FoundryTxType> {
    pub inner: EthTxResult<H, T>,
    pub sender: Address,
}

impl<H, T> TxResult for AnvilTxResult<H, T> {
    type HaltReason = H;

    fn result(&self) -> &ResultAndState<Self::HaltReason> {
        self.inner.result()
    }
}

/// Execution context for [`AnvilBlockExecutor`], providing block-level data
/// needed for pre/post execution system calls.
#[derive(Debug, Clone)]
pub struct AnvilExecutionCtx {
    /// Parent block hash — needed for EIP-2935 system call.
    pub parent_hash: B256,
    /// Whether Prague hardfork is active.
    pub is_prague: bool,
    /// Whether Cancun hardfork is active.
    pub is_cancun: bool,
}

/// Block executor for Anvil that implements [`BlockExecutor`].
///
/// Wraps an EVM instance and produces receipts via a pluggable [`AnvilReceiptBuilder`].
/// Validation (gas limits, blob gas, transaction validity) is handled by the
/// caller before transactions are fed to this executor.
pub struct AnvilBlockExecutor<E, RB: AnvilReceiptBuilder = FoundryReceiptBuilder> {
    /// The EVM instance used for execution.
    evm: E,
    /// Execution context.
    ctx: AnvilExecutionCtx,
    /// Receipt builder.
    receipt_builder: RB,
    /// Receipts of executed transactions.
    receipts: Vec<RB::Receipt>,
    /// Total gas used by transactions in this block.
    gas_used: u64,
    /// Blob gas used by the block.
    blob_gas_used: u64,
    /// Optional state change hook.
    state_hook: Option<Box<dyn OnStateHook>>,
}

impl<E: fmt::Debug, RB: AnvilReceiptBuilder> fmt::Debug for AnvilBlockExecutor<E, RB> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnvilBlockExecutor")
            .field("evm", &self.evm)
            .field("ctx", &self.ctx)
            .field("gas_used", &self.gas_used)
            .field("blob_gas_used", &self.blob_gas_used)
            .field("receipts", &self.receipts.len())
            .finish_non_exhaustive()
    }
}

impl<E, RB: AnvilReceiptBuilder> AnvilBlockExecutor<E, RB> {
    /// Creates a new [`AnvilBlockExecutor`].
    pub fn new(evm: E, ctx: AnvilExecutionCtx, receipt_builder: RB) -> Self {
        Self {
            evm,
            ctx,
            receipt_builder,
            receipts: Vec::new(),
            gas_used: 0,
            blob_gas_used: 0,
            state_hook: None,
        }
    }
}

impl<E, RB> BlockExecutor for AnvilBlockExecutor<E, RB>
where
    E: Evm<DB: StateDB, Tx: FromRecoveredTx<RB::Transaction> + FromTxWithEncoded<RB::Transaction>>,
    RB: AnvilReceiptBuilder,
{
    type Transaction = RB::Transaction;
    type Receipt = RB::Receipt;
    type Evm = E;
    type Result = AnvilTxResult<E::HaltReason, <RB::Transaction as TransactionEnvelope>::TxType>;

    fn apply_pre_execution_changes(&mut self) -> Result<(), BlockExecutionError> {
        // EIP-2935: store parent block hash in history storage contract.
        if self.ctx.is_prague {
            let result = self
                .evm
                .transact_system_call(
                    eip4788::SYSTEM_ADDRESS,
                    eip2935::HISTORY_STORAGE_ADDRESS,
                    Bytes::copy_from_slice(self.ctx.parent_hash.as_slice()),
                )
                .map_err(BlockExecutionError::other)?;

            if let Some(hook) = &mut self.state_hook {
                hook.on_state(
                    StateChangeSource::PreBlock(
                        alloy_evm::block::StateChangePreBlockSource::BlockHashesContract,
                    ),
                    &result.state,
                );
            }
            self.evm.db_mut().commit(result.state);
        }
        Ok(())
    }

    fn execute_transaction_without_commit(
        &mut self,
        tx: impl ExecutableTx<Self>,
    ) -> Result<Self::Result, BlockExecutionError> {
        let (tx_env, tx) = tx.into_parts();

        let block_available_gas = self.evm.block().gas_limit() - self.gas_used;
        if tx.tx().gas_limit() > block_available_gas {
            return Err(BlockValidationError::TransactionGasLimitMoreThanAvailableBlockGas {
                transaction_gas_limit: tx.tx().gas_limit(),
                block_available_gas,
            }
            .into());
        }

        let sender = *tx.signer();

        let result = self.evm.transact(tx_env).map_err(|err| {
            let hash = tx.tx().trie_hash();
            BlockExecutionError::evm(err, hash)
        })?;

        Ok(AnvilTxResult {
            inner: EthTxResult {
                result,
                blob_gas_used: tx.tx().blob_gas_used().unwrap_or_default(),
                tx_type: tx.tx().tx_type(),
            },
            sender,
        })
    }

    fn commit_transaction(&mut self, output: Self::Result) -> Result<u64, BlockExecutionError> {
        let AnvilTxResult {
            inner: EthTxResult { result: ResultAndState { result, state }, blob_gas_used, tx_type },
            sender,
        } = output;

        if let Some(hook) = &mut self.state_hook {
            hook.on_state(StateChangeSource::Transaction(self.receipts.len()), &state);
        }

        let gas_used = result.gas_used();
        self.gas_used += gas_used;

        if self.ctx.is_cancun {
            self.blob_gas_used = self.blob_gas_used.saturating_add(blob_gas_used);
        }

        let receipt = self.receipt_builder.build_receipt(AnvilReceiptBuilderCtx {
            tx_type,
            sender,
            evm: &self.evm,
            result,
            state: &state,
            cumulative_gas_used: self.gas_used,
        });

        self.receipts.push(receipt);
        self.evm.db_mut().commit(state);

        Ok(gas_used)
    }

    fn finish(self) -> Result<(Self::Evm, BlockExecutionResult<RB::Receipt>), BlockExecutionError> {
        Ok((
            self.evm,
            BlockExecutionResult {
                receipts: self.receipts,
                requests: Default::default(),
                gas_used: self.gas_used,
                blob_gas_used: self.blob_gas_used,
            },
        ))
    }

    fn set_state_hook(&mut self, hook: Option<Box<dyn OnStateHook>>) {
        self.state_hook = hook;
    }

    fn evm_mut(&mut self) -> &mut Self::Evm {
        &mut self.evm
    }

    fn evm(&self) -> &Self::Evm {
        &self.evm
    }

    fn receipts(&self) -> &[RB::Receipt] {
        &self.receipts
    }
}

pub struct AnvilBlockExecutorFactory;

impl AnvilBlockExecutorFactory {
    /// Generic constructor — custom networks pass their own `receipt_builder`.
    pub fn create_executor<DB, RB>(
        evm: EitherEvm<DB, AnvilInspector, PrecompilesMap>,
        ctx: AnvilExecutionCtx,
        receipt_builder: RB,
    ) -> AnvilBlockExecutor<EitherEvm<DB, AnvilInspector, PrecompilesMap>, RB>
    where
        DB: StateDB,
        RB: AnvilReceiptBuilder,
    {
        AnvilBlockExecutor::new(evm, ctx, receipt_builder)
    }

    /// Convenience wrapper using the default [`FoundryReceiptBuilder`].
    pub fn create_foundry_executor<DB>(
        evm: EitherEvm<DB, AnvilInspector, PrecompilesMap>,
        ctx: AnvilExecutionCtx,
    ) -> AnvilBlockExecutor<EitherEvm<DB, AnvilInspector, PrecompilesMap>>
    where
        DB: StateDB,
    {
        Self::create_executor(evm, ctx, FoundryReceiptBuilder)
    }
}

/// Builds the per-tx `OpTransaction<TxEnv>` from a pending transaction, replicating the logic
/// from `TransactionExecutor::env_for`.
pub fn build_tx_env_for_pending(
    tx: &PendingTransaction<FoundryTxEnvelope>,
    cheats: &CheatsManager,
    networks: NetworkConfigs,
    _evm_env: &EvmEnv,
) -> OpTransaction<TxEnv> {
    let mut tx_env: OpTransaction<TxEnv> =
        FromRecoveredTx::from_recovered_tx(tx.transaction.as_ref(), *tx.sender());

    if let FoundryTxEnvelope::Eip7702(tx_7702) = tx.transaction.as_ref()
        && cheats.has_recover_overrides()
    {
        let cheated_auths = tx_7702
            .tx()
            .authorization_list
            .iter()
            .zip(tx_env.base.authorization_list)
            .map(|(signed_auth, either_auth)| {
                either_auth.right_and_then(|recovered_auth| {
                    if recovered_auth.authority().is_none()
                        && let Ok(signature) = signed_auth.signature()
                        && let Some(override_addr) =
                            cheats.get_recover_override(&signature.as_bytes().into())
                    {
                        Either::Right(RecoveredAuthorization::new_unchecked(
                            recovered_auth.into_parts().0,
                            RecoveredAuthority::Valid(override_addr),
                        ))
                    } else {
                        Either::Right(recovered_auth)
                    }
                })
            })
            .collect();
        tx_env.base.authorization_list = cheated_auths;
    }

    if networks.is_optimism() {
        tx_env.enveloped_tx = Some(tx.transaction.encoded_2718().into());
    }

    tx_env
}

/// Creates a database with given database and inspector.
pub fn new_eth_evm_with_inspector<DB, I>(
    db: DB,
    env: &Env,
    inspector: I,
) -> EitherEvm<DB, I, PrecompilesMap>
where
    DB: Database<Error = DatabaseError> + Debug,
    I: Inspector<EthEvmContext<DB>> + Inspector<OpContext<DB>>,
{
    if env.networks.is_optimism() {
        let evm_env = EvmEnv::new(
            env.evm_env
                .cfg_env
                .clone()
                .with_spec_and_mainnet_gas_params(op_revm::OpSpecId::ISTHMUS),
            env.evm_env.block_env.clone(),
        );
        EitherEvm::Op(OpEvmFactory::default().create_evm_with_inspector(db, evm_env, inspector))
    } else {
        EitherEvm::Eth(EthEvmFactory::default().create_evm_with_inspector(
            db,
            env.evm_env.clone(),
            inspector,
        ))
    }
}
