# Alloy 2.0.0-rc.0 Migration

This document describes the full migration of Foundry from Alloy 1.4 to Alloy 2.0.0-rc.0 on the `alloy-2.0-rc` branch.

## Dependency Versions

| Dependency | Source | Rev / Branch |
|---|---|---|
| **alloy** | `alloy-rs/alloy` | rev `100a3325ac4d624f3eb0bd404125750e76edf8ca` |
| **alloy-evm** | `mablr/evm` | branch `chore/bump_alloy2.0_rc` |
| **op-alloy, alloy-op-evm, alloy-op-hardforks** | `mablr/optimism` | rev `1ebf2dc0d2fd86137d97b485c7994e7404744c87` |
| **revm-inspectors** | `stevencartavia/revm-inspectors` | branch `alloy-2.0` |
| **foundry-fork-db** | `stevencartavia/foundry-fork-db` | branch `alloy-2.0` |
| **tempo** | `stevencartavia/tempo` | branch `alloy-2.0` |
| **reqwest** | crates.io | `0.12` (workspace), alloy uses `0.13` internally |

> **reqwest version gap**: The workspace stays on `0.12` (required by `foundry-block-explorers`), but alloy uses `0.13` internally. Transport code uses `alloy_transport_http::reqwest` for type compatibility.

## Git Remotes

| Remote | URL |
|---|---|
| `origin` | `https://github.com/stevencartavia/foundry.git` |
| `upstream` | `https://github.com/foundry-rs/foundry.git` |
| `mablr` | `https://github.com/mablr/foundry.git` |
| `figtracer` | `https://github.com/figtracer/foundry.git` |

---

## Step-by-Step Migration

### Step 1: Fork Upstream Dependencies

Created `alloy-2.0` branches on personal forks for each dependency that needed alloy 2.0 compatibility:

- **`stevencartavia/revm-inspectors`** (`alloy-2.0`) — bumped alloy deps to `2.0.0-rc.0`
- **`stevencartavia/foundry-fork-db`** (`alloy-2.0`) — bumped alloy deps to `2.0.0-rc.0`
- **`stevencartavia/tempo`** (`alloy-2.0`) — bumped alloy deps, removed `reth` from default features in `tempo-primitives` to avoid dual-version alloy conflicts, implemented `SignableTransaction<Signature>` for `TempoTypedTransaction`

Later switched `alloy-evm` and `op-alloy` to mablr's forks which had the correct alloy 2.0 RC bumps.

### Step 2: Bump Alloy Dependencies in Foundry

**Commit:** `dee19737f chore: bump alloy dependencies to 2.0.0-rc.0`

- Bumped all `alloy-*` workspace dependencies from `"1.4"` to `"2.0.0-rc.0"` in `Cargo.toml`
- Uncommented and updated all `[patch.crates-io]` git overrides to point to the alloy rev `100a3325ac`
- Patched `alloy-evm`, `op-alloy`, `revm-inspectors`, `foundry-fork-db`, and `tempo` to their respective forks
- Applied initial code fixes for alloy 2.0 breaking API changes (see below)

### Step 3: Merge mablr & figtracer PRs (Generic Network Support)

These PRs make Foundry's crate architecture generic over `Network`, preparing for multi-network support:

#### Merge Order

| # | Branch | PR | Description |
|---|---|---|---|
| 1 | `mablr/feature/generalize_Signer_impl` | #13636 | Generalize `Signer` trait implementation for anvil |
| 2 | `figtracer/fig/cheatcodes-executor-inversion` | #13651 | Inversion of control for `CheatcodesExecutor` |
| 3 | `figtracer/fig/nested-evm-generic-factory` | #13652 | Make `NestedEvmExt` network-agnostic |
| 4 | `mablr/feature/generic_CastTxBuilder` | #13533 | Generic `CastTxBuilder` over `Network` |
| 5 | `mablr/feature/cast_estimate_generic_network` | #13622 | Cast `estimate` generic `Network` support |
| 6 | `mablr/feature/cast_generic_network_support` | #13624 | Cast generic `Network` support |
| 7 | `mablr/feature/cast_call_generic_network` | #13634 | Cast `call` generic `Network` support |
| 8 | `mablr/feature/cast_access_list_generic_network` | #13635 | Cast `access-list` generic `Network` support |
| 9 | `mablr/feature/generic_cast_send` | #13587 | Cast `send`+`erc20` generic `Network` support |
| 10 | `mablr/feature/evm_backend_generic_network` | #13579 | EVM `Backend` generic over `Network` |

Additional origin branches merged:
- `origin/generic-mined-transaction` — Make `MinedTransaction` and `ExecutedTransactions` generic over `Network`
- `origin/anvil-fork-network-generic` — Generalize anvil fork types over `Network`

### Step 4: Fix Compilation After PR Merges

**Commit:** `a18c2967b fix: resolve compilation errors after PR merges`

Fixed conflicts and compilation errors arising from merging multiple PRs:
- Removed duplicate test (`can_pretty_print_tempo_receipt`)
- Fixed import conflicts between overlapping PRs

### Step 5: Adapt `evm_backend_generic_network` (PR #13579)

This was the most complex PR to integrate. It was written against a newer alloy version that contained `DynTransactionBuilder` and `NetworkTransactionBuilder` traits — **these traits don't exist in our alloy rev** (`100a332`).

#### Created Two-Trait Architecture

Defined in `crates/primitives/src/network/transaction.rs`:

**`FoundryTxRequest`** — Object-safe trait for dynamic dispatch (`&dyn FoundryTxRequest`):
- Base field methods use `ftx_` prefix to avoid ambiguity with `TransactionBuilder` methods
- Methods: `ftx_from()`, `ftx_kind()`, `ftx_nonce()`, `ftx_value()`, `ftx_input()`, `ftx_gas_limit()`, `ftx_chain_id()`, `ftx_gas_price()`, `ftx_max_fee_per_gas()`, `ftx_max_priority_fee_per_gas()`, `ftx_access_list()`, `ftx_transaction_type()`
- Extension methods (no-op defaults): `max_fee_per_blob_gas`, `blob_versioned_hashes`, `blob_sidecar`, `authorization_list`, `fee_token`, `nonce_key`, plus `set_*` variants

**`FoundryTransactionBuilder<N: Network>`** — Sized builder trait:
- Extends: `FoundryTxRequest + TransactionBuilder<N>`
- Adds builder-pattern `with_*` methods for all Foundry-specific fields

#### Implementations

| Type | `FoundryTxRequest` | `FoundryTransactionBuilder<N>` |
|---|---|---|
| `TransactionRequest` | ✅ (via macro) | ✅ `<Ethereum>` |
| `WithOtherFields<TransactionRequest>` | ✅ (via macro) | ✅ `<AnyNetwork>` |
| `TempoTransactionRequest` | ✅ (manual) | ✅ `<TempoNetwork>` |
| `FoundryTransactionRequest` | ✅ (manual) | ✅ `<FoundryNetwork>` |

### Step 6: Fix Remaining Import Errors

**Commit:** `b5a614c71 fix: remove DynTransactionBuilder/NetworkTransactionBuilder imports, add FoundryTxRequest/TransactionBuilder imports`

Removed references to `DynTransactionBuilder` and `NetworkTransactionBuilder` from 9 files and added correct imports:

| File | Change |
|---|---|
| `crates/wallets/src/wallet_browser/mod.rs` | Removed `DynTransactionBuilder` |
| `crates/script/src/broadcast.rs` | Replaced with `TransactionBuilder` |
| `crates/script/src/simulate.rs` | Replaced with `TransactionBuilder` |
| `crates/anvil/src/eth/api.rs` | Removed both traits |
| `crates/anvil/src/eth/backend/mem/mod.rs` | Removed `NetworkTransactionBuilder`, added `TransactionBuilder` |
| `crates/anvil/tests/it/transaction.rs` | Removed `DynTransactionBuilder` |
| `crates/anvil/tests/it/gas.rs` | Removed `DynTransactionBuilder` |
| `crates/anvil/tests/it/eip4844.rs` | Removed `DynTransactionBuilder` |
| `crates/anvil/tests/it/optimism.rs` | Removed `NetworkTransactionBuilder` |
| `crates/cast/src/tx.rs` | Added `FoundryTxRequest` import |
| `crates/cast/src/cmd/call.rs` | Added `FoundryTxRequest` import |

---

## Alloy 2.0 Breaking API Changes

| Change | Details |
|---|---|
| **TransactionBuilder7594 removed** | Merged into `TransactionBuilder4844` |
| **BlobTransactionSidecar** | Replaced by `BlobTransactionSidecarVariant` (wrap with `::Eip4844()`) |
| **`set_blob_sidecar()`** | Now takes `BlobTransactionSidecarVariant` |
| **`SidecarBuilder::build()`** | Now generic; use `build_4844()` for EIP-4844 |
| **`TransactionBuilder::set_input`** | Now takes `T: Into<Bytes>` (generic) |
| **`TransactionBuilder::set_input_kind`** | Now takes `T: Into<Bytes>` (generic) |
| **`SimulateError`** | Gained a `data` field |
| **`TransactionInfo`** | Gained a `block_timestamp` field |
| **`Recovered` type** | Use `.into_inner()` instead of `.inner` field access |
| **`configure_tx_env`** | Now returns `Result` (callers need `?`) |
| **`Precompile` trait** | Lost the `is_pure()` method |

---

## Cargo Conflict Resolution

When merging PRs that conflict on `Cargo.toml` / `Cargo.lock`:

```bash
git checkout --ours Cargo.toml Cargo.lock
cargo generate-lockfile
```

This preserves the alloy 2.0 RC revisions and fork patches.

---

## Key Files Modified

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace dependency versions and `[patch.crates-io]` |
| `crates/primitives/src/network/transaction.rs` | `FoundryTxRequest` + `FoundryTransactionBuilder<N>` traits |
| `crates/primitives/src/network/mod.rs` | `FoundryNetwork` type + trait exports |
| `crates/primitives/src/transaction/request.rs` | `FoundryTransactionRequest` enum + `TransactionBuilder` impl |
| `crates/evm/core/src/utils.rs` | `configure_tx_env` and `configure_tx_req_env` |
| `crates/evm/core/src/backend/mod.rs` | `DatabaseExt::transact_from_tx` using `&dyn FoundryTxRequest` |
| `crates/cast/src/tx.rs` | `CastTxBuilder` generic over `Network` |
| `crates/cast/src/cmd/call.rs` | Cast `call` command |
| `crates/anvil/src/eth/api.rs` | Anvil API handlers |
| `crates/anvil/src/eth/backend/mem/mod.rs` | Anvil in-memory backend |
| `crates/script/src/broadcast.rs` | Forge script broadcasting |
| `crates/script/src/simulate.rs` | Forge script simulation |

### Step 7: Clean Up Warnings

Removed two remaining warnings to achieve a fully clean build:

- **`crates/cheatcodes/src/evm.rs`**: Removed unused `use alloy_rpc_types::TransactionRequest` import
- **`crates/cast/Cargo.toml`**: Removed unused `alloy-serde` dependency (only referenced in doc comments, not actual code)

## Build Status

✅ `cargo check --tests` passes with zero errors and zero warnings.
