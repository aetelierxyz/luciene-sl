# luciene-sl

Transparent and Stateless Agent for OnChain Financial Models.

---

## 🛰️ CEX → AWS Colocation Oracle

A service that probes the **public** REST/WSS APIs of twenty-one spot exchanges
(Binance, Coinbase, Kraken, Bybit, Bitfinex, OKX, Gemini, Bitget, Gate.io, KuCoin, HTX, MEXC, BitMart, Bitstamp, Crypto.com, Bitso, bitFlyer, Mercado Bitcoin, NDAX, Bitvavo, Bithumb), works out which
**AWS region** hosts each exchange's engine, and publishes an on-chain
(**Solana devnet**) recommendation of the best place to colocate a trading
server — a latitude/longitude point plus a **10 km radius**. A local dashboard
reads the recommendation **back from chain** and maps it.

- Architecture & methodology → [ARCHITECTURE.md](ARCHITECTURE.md)
- How to build / deploy / run (native + Docker) → [RUNBOOK.md](RUNBOOK.md)

```bash
# native, end-to-end (probe → publish on devnet → serve dashboard at :8080)
scripts/build.sh && scripts/run-all.sh
# or with Docker
SOLANA_KEYPAIR=$HOME/.config/solana/id.json docker compose up --build
```

Deployed oracle program (devnet): `GH9e1ZjDTRo4WKgJYxE1TZHSFFqgXxsDVTB4manBvvh1`

---

## The original on-chain model program (`luciene_sl`)

## Local testing

This is done in 3 steps: 

1. Configure and start a local validator.
2. Generate and fund a signer.
3. Validate and localy use the program. 

### In one terminal

Start from empty project 

```shel
cargo clean && anchor clean
```

Configure Solana CLI to use `localhost`, which usually would be `http://localhost::8099`

```shell
solana config set --url localhost
```

Start a local validator for testing purposes.

```shell
solana-test-validator
```

And this will run undefinitely until stopped. 

### In another terminal 

Make sure there is enough balance, 

```shell
solana balance

```

If not enough balance, you can airdrop yourself solana tokens.

```shell
solana airdrop 10

```

build the anchor program

```shell
anchor build
```

```shell
anchor deploy
```

Generate a signer to be the default

```shell
solana-keygen new -o target/deploy/luciene-keypair.json
```


## Devnet deployment

### Config 

Configure to devnet

```shell
solana config set --url devnet
```

validate 

```shell
solana config get
```

### Wallet and Funds

optionally, create a new wallet. 

```shell
solana-keygen new --outfile ~/.config/solana/id.json
```

Get the solana address

```shell
solana address
```

Fund the wallet with SOL (in devnet)

```shell
solana airdrop 5
```

Verify it was sucessfully created and additioned the new 5 SOL balance

```shell
solana balance
```

### Build and deploy program

Build the program

```shell
anchor build
```

Generate the program ID

```shell
solana address -k target/deploy/luciene-keypair.json
```

Now, update the program ID in the respective codes. 

- `programs/luciene-sl/src/lib.rs` in the `declare_id!("<PROGRAM_ID>")` line.
- `Anchor.toml` in the `lucien = "\<PROGRAM_ID\>"

Now, rebuild the program.

```shell
anchor build
```

Validate manually the `PROGRAM_ID` is correctly updated. Then, deploy to the `devnet`.

```shell
anchor deploy
```

Check the status of the program, now should be deployed. 

```shell
solana program show PROGRAM_ID
```

or look at the logs 

```shell
solana logs PROGRAM_ID
```

### Initialize, update and test program's functionality

In order to conduct tests, as they are defined in `/test` run the following command 
with the `--skip-local-validator` flag in order to avoid the full cycle of build, deploy, 
test and shutdown. In favor for a more granular sequence. 

```shell
anchor test --skip-local-validator
```
https://explorer.solana.com/
