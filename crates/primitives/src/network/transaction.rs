use alloy_consensus::{
    BlobTransactionSidecar, BlobTransactionSidecarEip7594, BlobTransactionSidecarVariant,
};
use alloy_network::{AnyNetwork, Ethereum, Network, TransactionBuilder};
use alloy_primitives::{Address, Bytes, B256, TxKind, U256};
use alloy_rpc_types::{AccessList, SignedAuthorization, TransactionRequest};
use alloy_serde::WithOtherFields;
use std::fmt::Debug;
use tempo_alloy::{TempoNetwork, rpc::TempoTransactionRequest};

/// Object-safe transaction request trait for Foundry.
///
/// This provides dyn-safe access to transaction request fields, enabling use as `&dyn
/// FoundryTxRequest` in the EVM backend and other contexts that need dynamic dispatch.
///
/// It includes base transaction fields (from, nonce, gas, etc.) plus Foundry-specific
/// extensions for EIP-4844 blob transactions, EIP-7702 authorization lists, and Tempo
/// transactions.
///
/// By default, extension methods have no-op implementations.
pub trait FoundryTxRequest: Debug {
    // ── Base transaction fields ──────────────────────────────────────────

    /// Get the sender address.
    fn ftx_from(&self) -> Option<Address> {
        None
    }

    /// Get the transaction kind (to address or create).
    fn ftx_kind(&self) -> Option<TxKind> {
        None
    }

    /// Get the nonce.
    fn ftx_nonce(&self) -> Option<u64> {
        None
    }

    /// Get the value.
    fn ftx_value(&self) -> Option<U256> {
        None
    }

    /// Get the input data.
    fn ftx_input(&self) -> Option<&Bytes> {
        None
    }

    /// Get the gas limit.
    fn ftx_gas_limit(&self) -> Option<u64> {
        None
    }

    /// Get the chain ID.
    fn ftx_chain_id(&self) -> Option<u64> {
        None
    }

    /// Get the gas price (legacy/EIP-2930).
    fn ftx_gas_price(&self) -> Option<u128> {
        None
    }

    /// Get the max fee per gas (EIP-1559).
    fn ftx_max_fee_per_gas(&self) -> Option<u128> {
        None
    }

    /// Get the max priority fee per gas (EIP-1559).
    fn ftx_max_priority_fee_per_gas(&self) -> Option<u128> {
        None
    }

    /// Get the access list.
    fn ftx_access_list(&self) -> Option<&AccessList> {
        None
    }

    /// Get the transaction type.
    fn ftx_transaction_type(&self) -> Option<u8> {
        None
    }

    // ── EIP-4844 blob fields ──────────────────────────────────────────

    /// Get the max fee per blob gas for the transaction.
    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        None
    }

    /// Set the max fee per blob gas for the transaction.
    fn set_max_fee_per_blob_gas(&mut self, _max_fee_per_blob_gas: u128) {}

    /// Gets the EIP-4844 blob versioned hashes of the transaction.
    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        None
    }

    /// Sets the EIP-4844 blob versioned hashes of the transaction.
    fn set_blob_versioned_hashes(&mut self, _hashes: Vec<B256>) {}

    /// Gets the blob sidecar (either EIP-4844 or EIP-7594 variant) of the transaction.
    fn blob_sidecar(&self) -> Option<&BlobTransactionSidecarVariant> {
        None
    }

    /// Sets the blob sidecar (either EIP-4844 or EIP-7594 variant) of the transaction.
    fn set_blob_sidecar(&mut self, _sidecar: BlobTransactionSidecarVariant) {}

    /// Gets the EIP-4844 blob sidecar if the current sidecar is of that variant.
    fn blob_sidecar_4844(&self) -> Option<&BlobTransactionSidecar> {
        self.blob_sidecar().and_then(|s| s.as_eip4844())
    }

    /// Sets the EIP-4844 blob sidecar of the transaction.
    fn set_blob_sidecar_4844(&mut self, sidecar: BlobTransactionSidecar) {
        self.set_blob_sidecar(BlobTransactionSidecarVariant::Eip4844(sidecar));
    }

    /// Gets the EIP-7594 blob sidecar if the current sidecar is of that variant.
    fn blob_sidecar_7594(&self) -> Option<&BlobTransactionSidecarEip7594> {
        self.blob_sidecar().and_then(|s| s.as_eip7594())
    }

    /// Sets the EIP-7594 blob sidecar of the transaction.
    fn set_blob_sidecar_7594(&mut self, sidecar: BlobTransactionSidecarEip7594) {
        self.set_blob_sidecar(BlobTransactionSidecarVariant::Eip7594(sidecar));
    }

    // ── EIP-7702 fields ──────────────────────────────────────────

    /// Get the EIP-7702 authorization list for the transaction.
    fn authorization_list(&self) -> Option<&Vec<SignedAuthorization>> {
        None
    }

    /// Sets the EIP-7702 authorization list.
    fn set_authorization_list(&mut self, _authorization_list: Vec<SignedAuthorization>) {}

    // ── Tempo fields ──────────────────────────────────────────

    /// Get the fee token for a Tempo transaction.
    fn fee_token(&self) -> Option<Address> {
        None
    }

    /// Set the fee token for a Tempo transaction.
    fn set_fee_token(&mut self, _fee_token: Address) {}

    /// Get the 2D nonce key for a Tempo transaction.
    fn nonce_key(&self) -> Option<U256> {
        None
    }

    /// Set the 2D nonce key for a Tempo transaction.
    fn set_nonce_key(&mut self, _nonce_key: U256) {}
}

/// Sized transaction builder trait for Foundry transactions.
///
/// This extends [`TransactionBuilder<N>`] (for network-specific building), adding
/// builder-pattern methods for Foundry-specific fields like blob sidecars, authorization
/// lists, and Tempo transaction fields.
pub trait FoundryTransactionBuilder<N: Network>: FoundryTxRequest + TransactionBuilder<N> {
    /// Builder-pattern method for setting max fee per blob gas.
    fn with_max_fee_per_blob_gas(mut self, max_fee_per_blob_gas: u128) -> Self {
        self.set_max_fee_per_blob_gas(max_fee_per_blob_gas);
        self
    }

    /// Builder-pattern method for setting the EIP-4844 blob versioned hashes.
    fn with_blob_versioned_hashes(mut self, hashes: Vec<B256>) -> Self {
        self.set_blob_versioned_hashes(hashes);
        self
    }

    /// Builder-pattern method for setting the blob sidecar of the transaction.
    fn with_blob_sidecar(mut self, sidecar: BlobTransactionSidecarVariant) -> Self {
        self.set_blob_sidecar(sidecar);
        self
    }

    /// Builder-pattern method for setting the EIP-4844 blob sidecar of the transaction.
    fn with_blob_sidecar_4844(mut self, sidecar: BlobTransactionSidecar) -> Self {
        self.set_blob_sidecar_4844(sidecar);
        self
    }

    /// Builder-pattern method for setting the EIP-7594 blob sidecar of the transaction.
    fn with_blob_sidecar_7594(mut self, sidecar: BlobTransactionSidecarEip7594) -> Self {
        self.set_blob_sidecar_7594(sidecar);
        self
    }

    /// Builder-pattern method for setting the authorization list.
    fn with_authorization_list(mut self, authorization_list: Vec<SignedAuthorization>) -> Self {
        self.set_authorization_list(authorization_list);
        self
    }

    /// Builder-pattern method for setting the Tempo fee token.
    fn with_fee_token(mut self, fee_token: Address) -> Self {
        self.set_fee_token(fee_token);
        self
    }

    /// Builder-pattern method for setting a 2D nonce key for a Tempo transaction.
    fn with_nonce_key(mut self, nonce_key: U256) -> Self {
        self.set_nonce_key(nonce_key);
        self
    }
}

// ── Implementations ──────────────────────────────────────────

/// Helper macro to implement FoundryTxRequest base fields for types backed by TransactionRequest.
macro_rules! impl_foundry_tx_request_for_tx_req {
    () => {
        fn ftx_from(&self) -> Option<Address> {
            self.from
        }
        fn ftx_kind(&self) -> Option<TxKind> {
            self.to
        }
        fn ftx_nonce(&self) -> Option<u64> {
            self.nonce
        }
        fn ftx_value(&self) -> Option<U256> {
            self.value
        }
        fn ftx_input(&self) -> Option<&Bytes> {
            self.input.input()
        }
        fn ftx_gas_limit(&self) -> Option<u64> {
            self.gas
        }
        fn ftx_chain_id(&self) -> Option<u64> {
            self.chain_id
        }
        fn ftx_gas_price(&self) -> Option<u128> {
            self.gas_price
        }
        fn ftx_max_fee_per_gas(&self) -> Option<u128> {
            self.max_fee_per_gas
        }
        fn ftx_max_priority_fee_per_gas(&self) -> Option<u128> {
            self.max_priority_fee_per_gas
        }
        fn ftx_access_list(&self) -> Option<&AccessList> {
            self.access_list.as_ref()
        }
        fn ftx_transaction_type(&self) -> Option<u8> {
            self.transaction_type
        }
    };
}

impl FoundryTxRequest for TransactionRequest {
    impl_foundry_tx_request_for_tx_req!();

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        self.max_fee_per_blob_gas
    }

    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128) {
        self.max_fee_per_blob_gas = Some(max_fee_per_blob_gas);
    }

    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        self.blob_versioned_hashes.as_deref()
    }

    fn set_blob_versioned_hashes(&mut self, hashes: Vec<B256>) {
        self.blob_versioned_hashes = Some(hashes);
    }

    fn blob_sidecar(&self) -> Option<&BlobTransactionSidecarVariant> {
        self.sidecar.as_ref()
    }

    fn set_blob_sidecar(&mut self, sidecar: BlobTransactionSidecarVariant) {
        self.sidecar = Some(sidecar);
        self.populate_blob_hashes();
    }

    fn authorization_list(&self) -> Option<&Vec<SignedAuthorization>> {
        self.authorization_list.as_ref()
    }

    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>) {
        self.authorization_list = Some(authorization_list);
    }
}

impl FoundryTransactionBuilder<Ethereum> for <Ethereum as Network>::TransactionRequest {}

impl FoundryTxRequest for WithOtherFields<TransactionRequest> {
    impl_foundry_tx_request_for_tx_req!();

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        self.max_fee_per_blob_gas
    }

    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128) {
        self.max_fee_per_blob_gas = Some(max_fee_per_blob_gas);
    }

    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        self.blob_versioned_hashes.as_deref()
    }

    fn set_blob_versioned_hashes(&mut self, hashes: Vec<B256>) {
        self.blob_versioned_hashes = Some(hashes);
    }

    fn blob_sidecar(&self) -> Option<&BlobTransactionSidecarVariant> {
        self.sidecar.as_ref()
    }

    fn set_blob_sidecar(&mut self, sidecar: BlobTransactionSidecarVariant) {
        self.sidecar = Some(sidecar);
        self.populate_blob_hashes();
    }

    fn authorization_list(&self) -> Option<&Vec<SignedAuthorization>> {
        self.authorization_list.as_ref()
    }

    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>) {
        self.authorization_list = Some(authorization_list);
    }
}

impl FoundryTransactionBuilder<AnyNetwork> for <AnyNetwork as Network>::TransactionRequest {}

impl FoundryTxRequest for TempoTransactionRequest {
    fn ftx_from(&self) -> Option<Address> {
        self.inner.from
    }
    fn ftx_kind(&self) -> Option<TxKind> {
        self.inner.to
    }
    fn ftx_nonce(&self) -> Option<u64> {
        self.inner.nonce
    }
    fn ftx_value(&self) -> Option<U256> {
        self.inner.value
    }
    fn ftx_input(&self) -> Option<&Bytes> {
        self.inner.input.input()
    }
    fn ftx_gas_limit(&self) -> Option<u64> {
        self.inner.gas
    }
    fn ftx_chain_id(&self) -> Option<u64> {
        self.inner.chain_id
    }
    fn ftx_gas_price(&self) -> Option<u128> {
        self.inner.gas_price
    }
    fn ftx_max_fee_per_gas(&self) -> Option<u128> {
        self.inner.max_fee_per_gas
    }
    fn ftx_max_priority_fee_per_gas(&self) -> Option<u128> {
        self.inner.max_priority_fee_per_gas
    }
    fn ftx_access_list(&self) -> Option<&AccessList> {
        self.inner.access_list.as_ref()
    }
    fn ftx_transaction_type(&self) -> Option<u8> {
        self.inner.transaction_type
    }

    fn authorization_list(&self) -> Option<&Vec<SignedAuthorization>> {
        self.authorization_list.as_ref()
    }

    fn set_authorization_list(&mut self, authorization_list: Vec<SignedAuthorization>) {
        self.authorization_list = Some(authorization_list);
    }

    fn fee_token(&self) -> Option<Address> {
        self.fee_token
    }

    fn set_fee_token(&mut self, fee_token: Address) {
        self.fee_token = Some(fee_token);
    }

    fn nonce_key(&self) -> Option<U256> {
        self.nonce_key
    }

    fn set_nonce_key(&mut self, nonce_key: U256) {
        self.nonce_key = Some(nonce_key);
    }
}

impl FoundryTransactionBuilder<TempoNetwork> for <TempoNetwork as Network>::TransactionRequest {}
