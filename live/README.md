# Live Esmeralda run — status

## What is built

`live/` is a small Rust CLI (`invisible-payment-live`) modelled on ajo-cli
(github.com/G-ojies/ajo-ootle), which proved this route on Esmeralda.
It uses ootle-rs **0.32** with tari_ootle_transaction/common_types **0.50**
— ajo's exact, proven-live versions — rather than the 0.6.2 API from the
docs, whose `take_free_coins` shape differs from ajo's proven
`take_faucet_funds()` call. Build: `cargo build --release` (10m 18s first
build; one unused-import warning, harmless).

Commands: `new`, `whoami`, `faucet`, `balance`, `publish-dry`,
`publish`, `hide`, `reveal`, `cancel`. Every transaction is dry-run first
and only sent if the dry run commits, same as ajo-cli.

## Keys (testnet only, never mainnet)

Fresh Esmeralda keys in `live/keys/` (mode 600): `sender.key`,
`receiver.key`. The secret values are never printed anywhere. Addresses
are public and safe to share:

- sender:   otl_esm_1yjynncqv4lrjzee2h3k02pv0twvkpnpfw9kjlgd2r3szu78hze7xdqgqphax2n628vdwen54lum96e32q8yy9jvhq67ef3tmlmm3kdghhhsuc
- receiver: otl_esm_1prnrdtm3fcjcy2ky2tk50qhyc05h7qe32s56c4p89gjd4ytzh33myh86e3p0qkkv2qklmgrrqfl88ak655h0nlrnv4qfrjpqdlcqylcf0s2uh

## STOPPED at step 3: the faucet is dry

2026-11-19, epoch 13234. Both faucet claims were sent and **rejected
on-chain** with the identical error:

`Reject(ExecutionFailure { code: InsufficientFunds, message: "At instruction #3: Resource did not contain sufficient balance: Bucket or vault contained insufficient revealed funds. Required: 1000000000, Available: 0" })`

- receiver claim tx: 55a49bed44273482f7ddb69df760579e81ae691ccf936c2ffea86f68218d8574
- sender claim tx (retry after a transient TLS drop to the indexer): 930ea395e4c53600ec7d331872853e997b8ebea88dcada91b5379ec6f33163c5

The faucet template holds 0 tTARI, so no account can claim. Balance checks
confirm both accounts hold nothing: their account components return 404
from the indexer (never created). This is the same dry-faucet failure
reported by other builders (5 Nov 2026: `Required: 1000000000,
Available: 0`).

## Not done (blocked on funding)

- Dry-run publish + publish of `invisible_payment.wasm` (136,334 bytes)
- Real hide-and-reveal with the published template
- Nothing was published, no template address exists yet, no contest
  submission was made, `demo/config.js` was not touched.

## Next options (each needs a fresh yes from Eddie)

1. **Burn-and-claim**: CPU-mine testnet Minotari (tXTM), burn it over the
   burn/claim bridge, claim tTARI. Heavy; needs a machine running the
   miner, and the borrowed desktop is not ours to load.
2. **Community tap**: ask in the Tari community (forum/Discord) for a
   small amount of testnet TARI to the sender address above. Ajo's
   publish cost 2.13 tTARI and each call ~0.004 tTARI, so ~5 tTARI
   covers publish + hide + reveal with headroom.
3. **Wait and retry the faucet**: it has been refilled before after
   going dry; re-run `./target/release/invisible-payment-live faucet sender`
   later and check the balance.

Once either account is funded, the run resumes with:
`publish-dry sender ../target/wasm32-unknown-unknown/release/invisible_payment.wasm`
then `publish`, then `hide`, poll the epoch, then `reveal`.
