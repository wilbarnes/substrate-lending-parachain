# Lending Runtime Proof of Concept

Open finance is a concept that strongly resonates with me. Additionally, having the liberty to write code so close to the machinery of a chain is something worth exploring. 

## The Runtime

The logic is simple, and for the sake of brevity, much of the logic has been generalized. This proof-of-concept was build with speed with the goal of iterating on it over time. It is not production ready. 

The liquidity provider used is Alice, and this variable is set using the GenesisConfig with the variable being retrieved from the 'src/chain_spec.rs' file. 

Within the 'src/chain_spec.rs' file:
```
impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		Ok(match self {
			[..cut..]
			Alternative::LocalTestnet => ChainSpec::from_genesis(
				"Local Testnet",
				"local_testnet",
				|| testnet_genesis(vec![
					authority_key("Alice"),
					authority_key("Bob"),
				], vec![
					account_key("Alice"), // adding additional accounts 
					account_key("Bob"),   // that we'll later outfit with currency
					account_key("Charlie"),
					account_key("Dave"),
					account_key("Eve"),
					account_key("Ferdie"),
				],
					account_key("Alice"),
				),
				vec![],
				[..cut..]
			),
		})
	}

```

```
fn testnet_genesis(initial_authorities: Vec<AuthorityId>, endowed_accounts: Vec<AccountId>, root_key: AccountId) -> GenesisConfig {
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/lending_runtime_wasm.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		timestamp: Some(TimestampConfig {
			minimum_period: 5, // 10 second block time.
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 0,
			existential_deposit: 500,
			transfer_fee: 0,
			creation_fee: 0,
			balances: endowed_accounts.iter().cloned().map(|k|(k, 1_000_000)).collect(),
			vesting: vec![],
		}),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
                lending: Some(LendingConfig {
                    liquidity_provider: account_key("Alice"),
                }),
	}
}
```

Methods:
```
// supplying currency to the runtime
fn deposit(_origin, deposit_value: T::Balance) -> Result {};
fn withdraw_in_full(_origin) -> Result {};

// borrowing currency from the runtime
fn borrow(_origin, borrow_value: T::Balance) -> Result {};
fn repay_in_full(_origin) -> Result ();

fn on_finalize() {};
```

- Users supplying currency to Alice compound interest at 1% per block. That's quite a nice rate to compound on a per block basis. 
- Users borrowing currency from Alice compound interest at 25% per block. Quite an expensive loan. 
- If Alice garners some borrowers she'll be earning good cash. However, her intention is to act as a market maker and she's saved an initial 1,000,000 units of currency to bootstrap her market making operation, so she's looking for folks to supply some additional cash. This is how she'll scale and earn more currency. 

Supplying and Earning Interest 
- Using the 'deposit()' method, any user can supply currency and start collecting interest from Alice, our liquidity provider. 
- Using the 'withdraw_in_full()' method, any user with a deposit can exit the market collecting their initial stake and any accrued interest. 

Borrowing and Repaying Interest
- Using the 'borrow()' method, any user can borrow currency and start having the interest they'll eventually pay back start compounding. It's an expensive loan, 25%, so don't borrow and forget!
- Using the 'repay_in_full()' method, any user who's borrowed currency can repay it back in addition to any interest they owe. 

NOTE: Currently, this is an unsecured loan. Let's make the assumption that Alice knows or has vetted the folks she's allowing to borrow from her. 
DEV TODO: Implement logic for secured lending (akin to MakerDAO & Compound).




The runtime constructed here is a Proof-of-Concept, intended solely for instructional purposes at this time, though these are use-cases I will implement over time. 



### Create the lending runtime module
```
substrate-module-new lending wil
```

### We make some initial changes in 'chain_spec.rs'
Our chain specification is found here: 'src/chain_spec.rs' 

To our dev chain, we add three additional accounts (Bob, Charlie, and Dave) that we will outfit with 1_000_000 units of currency. 

# Building

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Install required tools:

```bash
./scripts/init.sh
```

Build the WebAssembly binary:

```bash
./scripts/build.sh
```

Build all native code:

```bash
cargo build
```

# Run

You can start a development chain with:

```bash
cargo run -- --dev
```

Detailed logs may be shown by running the node with the following environment variables set: `RUST_LOG=debug RUST_BACKTRACE=1 cargo run -- --dev`.

If you want to see the multi-node consensus algorithm in action locally, then you can create a local testnet with two validator nodes for Alice and Bob, who are the initial authorities of the genesis chain that have been endowed with testnet units. Give each node a name and expose them so they are listed on the Polkadot [telemetry site](https://telemetry.polkadot.io/#/Local%20Testnet). You'll need two terminal windows open.

We'll start Alice's substrate node first on default TCP port 30333 with her chain database stored locally at `/tmp/alice`. The bootnode ID of her node is `QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN`, which is generated from the `--node-key` value that we specify below:

```bash
cargo run -- \
  --base-path /tmp/alice \
  --chain=local \
  --alice \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --telemetry-url ws://telemetry.polkadot.io:1024 \
  --validator
```

In the second terminal, we'll start Bob's substrate node on a different TCP port of 30334, and with his chain database stored locally at `/tmp/bob`. We'll specify a value for the `--bootnodes` option that will connect his node to Alice's bootnode ID on TCP port 30333:

```bash
cargo run -- \
  --base-path /tmp/bob \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/QmQZ8TjTqeDj3ciwr93EJ95hxfDsb9pEYDizUAbWpigtQN \
  --chain=local \
  --bob \
  --port 30334 \
  --telemetry-url ws://telemetry.polkadot.io:1024 \
  --validator
```

Additional CLI usage options are available and may be shown by running `cargo run -- --help`.
