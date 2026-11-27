# The Invisible Payment

**Money nobody can see until the receiver reveals it.**

**10/10 engine tests pass — real Tari engine, no mocks.**

> **Demo video:** TODO — not filmed yet, and not strictly required for this contest. A 60-second SIMULATION walkthrough script is in `VIDEO-SCRIPT.md` if Eddie films one for the announcement post.
>
> **Submission:** reply to the contest thread at https://community.tari.com/t/november-build-contest-thread-mvp/520 — deadline 1 Dec 2026, 01:00 WAT. Payment address received 26 Nov 2026; announcement post live on Medium 26 Nov 2026. Thread reply posted and live as post 47 (27 Nov 2026); live on-chain reveal done the same day (see below).
>
> **Live demo (simulation):** https://skymiller1509-blip.github.io/invisible-payment/

A stealth payment on Tari Ootle. Money nobody can see until the receiver
reveals it. Built for the Tari November Build Contest (MVP theme).

## Hackathon Requirements Met

The contest reply must carry five things (official rules). Each one, with proof:

- [x] Publicly viewable code link — this repo, public since 22 Nov 2026 (17 files, no keys).
- [x] MVP app link — live simulation demo: https://skymiller1509-blip.github.io/invisible-payment/
- [x] Project description — in the thread reply draft (`../contest-thread-draft.md`).
- [x] Tari payment address — mainnet L1 address received 26 Nov 2026 (in the reply draft).
- [x] Public announcement post — live on Medium, 26 Nov 2026: https://medium.com/@poppyemerald3/the-invisible-payment-a-stealth-vault-on-tari-a1a0805c15cb (Medium counts; X does not count this month).
- [x] Thread reply posted — live as post 47 of 52 in the contest thread (by "Ihu", 26 Nov 2026; staff review cleared 27 Nov).
- [x] Live on-chain reveal — DONE 27 Nov 2026: template `template_7cc97df35dd28e88dc3932e9fc0a2e49fd59b84d760defa1b4482b72a6e3746d` published, 1 TARI locked in the vault, revealed at epoch 13449, tx `06595f82498fc7942314b75e8c36d43d59602d8fa5e3243788019639febf3c93`, fee 3726 µT; receiver balance after 1996274 µT. Funded by the entrant herself — the Esmeralda faucet stayed dry.

Demo failure-proofing: the simulation needs no wallet, no faucet and no network, so it cannot break on camera. The live path is now proven on-chain too (template published, 1 TARI locked and revealed, 27 Nov 2026).

## Judging Highlights

- **Originality:** a stealth payment vault — no amount, no sender, no receiver visible on chain until the receiver reveals it with the secret. The invisibility is the native stealth property of TARI on Ootle, not a mock.
- **Technical execution:** 10/10 engine tests pass against the real Tari engine in-process, no mocks; the lock is three real conditions (reveal epoch, SHA-256 secret, receiver key); rejected reveals commit nothing.
- **Completeness:** the full flow works end to end — hide, cancel, reveal, and all three refusal paths — proven in the engine tests, shown in the live simulation demo, and proven live on Esmeralda 27 Nov 2026 (vault locked 1 TARI, receiver revealed it, tx 06595f82…, epoch 13449).
- **Presentation:** the demo labels itself a simulation throughout; the announcement post is live on Medium; the thread reply is drafted with every required link.

## What it is

A sender locks TARI in a vault. On Ootle, native TARI is a stealth
resource: the amount and the owner are hidden from observers. The payment
opens only after a reveal epoch, only to the named receiver, and only
with the secret phrase. Until the receiver reveals it with the secret,
there is nothing to see — no amount, no sender, no receiver. Then the
balance appears.

The lock is three real conditions — time (`AfterEpoch`, read as the
current epoch), secret (SHA-256 of the phrase), receiver's key — and the
reveal is the moment the money becomes visible.

## The flow

1. **Sender hides a payment.** Deposits TARI, names the receiver's
   public key, sets a reveal epoch, stores SHA-256(secret). The secret
   itself never touches the chain.
2. **Sender can cancel** any time before the reveal epoch and reclaim
   the funds in full.
3. **After the reveal epoch, the receiver reveals with the secret.**
   The template checks the epoch, checks the caller's key is the
   receiver's, checks SHA-256(secret) against the stored hash, then
   pays the receiver's account in full — and the payment becomes
   visible as a balance.
4. Reveals by anyone else, reveals with a wrong secret, and reveals
   before the epoch are all rejected, and a rejected transaction
   commits nothing.

## Tech stack

- Rust WASM template for Tari Ootle (Esmeralda testnet), tested with `tari_template_test_tooling` (real engine, in-process, no mocks)
- Demo: one static page (`demo/index.html` + `demo/config.js`, no build step, no backend), simulation mode plus a wired live mode via `window.tari`

## Run the tests

The tests run against the real Tari engine in-process
(`tari_template_test_tooling`), no mocks, no network.

```sh
cargo test
```

10 tests, all passing:

- `create_locks_funds_and_records_terms` — payment holds the deposit,
  terms recorded, sender funded it.
- `create_rejects_short_hash` — a non-32-byte hash is refused.
- `create_rejects_past_reveal_epoch` — the reveal epoch must be in the
  future.
- `claim_blocked_before_reveal_epoch` — even the receiver with the
  right secret cannot reveal early.
- `claim_blocked_with_wrong_secret` — a wrong phrase is refused.
- `claim_blocked_for_non_receiver` — the right secret with the wrong
  signer is refused.
- `claim_succeeds_after_epoch_with_right_secret` — after the epoch, the
  receiver reveals, the vault empties, the receiver's balance rises by
  exactly the locked amount, and a second reveal is refused.
- `cancel_returns_funds_before_epoch` — the sender reclaims in full,
  and the receiver can no longer reveal.
- `cancel_blocked_after_reveal_epoch` — after the reveal epoch the
  sender cannot take the funds back.
- `cancel_blocked_for_non_sender` — a stranger cannot cancel
  (`AccessDenied` from the owner rule).

## Try it yourself

Open the live demo (simulation — no wallet, no real TARI moves):
https://skymiller1509-blip.github.io/invisible-payment/

Or serve the `demo/` folder locally:

```sh
cd demo
python3 -m http.server 8000
```

Then open <http://localhost:8000>.

**Simulation (default).** A walletless simulation of the same flow, for
judges without a wallet. The show is the reveal: the page first shows
the vault as invisible (no amount, no sender, no receiver), then at the
reveal time the receiver types the secret and the balance appears. It
also walks through reveal blocked before the epoch, reveal blocked with
a wrong secret, and the cancel path on a second payment. The page labels
itself as a simulation throughout.

**On-chain (live).** Below the simulation. Connect a Sapient wallet (or
open the page inside Tari Universe) on Esmeralda and it talks to the
published template through `window.tari`: connect, read the current
epoch from the public indexer, hide a payment with a stealth withdrawal
(`InvisiblePayment.create`), look a payment up from the indexer, reveal
it (`claim`) or cancel it (`cancel`). Real test TARI moves, and every
step asks for approval in the wallet first. Until a template address is
pasted into `demo/config.js`, the live mode can connect and read the
epoch but cannot lock funds — and it says so on the page instead of
pretending a live run happened.

## Build the WASM template

```sh
cargo build --target wasm32-unknown-unknown --release
```

The binary lands in
`target/wasm32-unknown-unknown/release/invisible_payment.wasm`.

## Publish to Esmeralda (not done here)

Publishing needs a funded Sapient wallet on the Esmeralda testnet, which
this build does not have, so the template is **not** published live. To
publish it yourself, build the WASM above, then in the Sapient wallet on
Esmeralda go to the Home page, click "Publish Template", upload the
`.wasm`, estimate the fee first, and publish. (The official guide is
<https://ootle.tari.com/guides/publishing-templates/>.) Paste the
`template_...` address it returns into `demo/config.js` — that is the
only change the live mode needs. A live reveal on Esmeralda — real TARI
moving from an invisible vault into the receiver's balance — is the step
that makes this entry mind-blowing instead of a simulation.

## Live mode: what it calls, and the open questions

Every live-mode call comes from the official integration docs
(<https://universe.tari.mw/integration> and its typed definitions at
<https://universe.tari.mw/integration/tari-dapp.d.ts>) or from the
November contest entrant "stake" (<https://github.com/Mzee04/stake>,
`web/app.js`), which ships the same pattern: connect, read the current
epoch from the public indexer (`GET https://ootle-indexer-a.tari.com/network`,
verified live 2026-11-19: `{"network":"esmeralda","epoch":13233}`), call
a template function with a stealth withdrawal, look a component up from
the indexer. The exact template calls, in the template's own argument
order (`src/lib.rs`):

- Hide: `tari_createTransactionRequest` with
  `kind: "withdrawStealthAndExecute"`, the XTR stealth resource address
  (found at runtime from `tari_getBalances`), the amount in micro-TARI
  as a string, and one follow-up instruction:
  `CallFunction { address: <bare template hex>, function: "create",
  args: [workspace bucket, receiver key (32 bytes), reveal epoch,
  SHA-256(secret) (32 bytes)] }`. The receiver key and the secret hash
  are untagged 32-byte CBOR strings; the epoch is a CBOR unsigned int.
- Reveal: `tari_createTransactionRequest` with `kind: "instructions"`,
  one instruction: `CallMethod { call: { Address: <component> },
  method: "claim", args: [secret (bytes), payout account (component)] }`.
  The payout is the connected wallet's own account.
- Cancel: same, with three instructions: `CallMethod cancel`, then
  `PutLastInstructionOutputOnWorkspace { key: 0 }`, then `CallMethod
  deposit` of the returned bucket into the sender's account (the
  documented pattern from the integration docs' "Transacting" section).
- Look up: `GET {indexer}/substates/<component address>`; the component
  state fields are read in the order declared in `src/lib.rs` (sender,
  receiver, funds vault, reveal epoch, secret hash, status).

Open questions, marked TODO in the demo source — nothing here is
guessed:

- **TODO-1 `workspaceVarName`.** The official `.d.ts` lists
  `workspaceVarName` as required for `withdrawStealthAndExecute`, but
  no example value appears anywhere in the integration docs, and
  stake's code omits the field. This code omits it too; if the wallet
  rejects the request for a missing field, add it per the wallet's
  error.
- **TODO-2 receiver public key.** The provider does not expose the
  connected account's public key, so the receiver's 64-hex key must be
  pasted into the live form. The docs reference a `parseOotleAddress`
  decoder in the paylink dApp but do not publish its code in the
  integration docs.
- **TODO-3 workspace bucket id.** The docs say
  `withdrawStealthAndExecute` reveals the bucket at workspace id 1
  (and that id 0 fails); stake's code uses id 0. This code follows the
  docs (id 1).
- **New component address.** The create result carries a transaction
  id; the new component address is taken from the transaction result or
  the explorer and pasted into "Look up a payment" (the demo fills it
  in automatically when the result includes one).

## Contest submission

Reply to the November contest thread
(<https://community.tari.com/t/november-build-contest-thread-mvp/520>)
with the code link, the MVP app link, a description, a Tari payment
address, and a link to a public announcement post. Note the November
rules: the announcement post must be on Bluesky, Reddit, Medium or
YouTube — X does not count this month.

## Honest limits

- **The simulation is a simulation.** No real TARI moves in the browser
  simulation. The on-chain template is the Rust code in `src/lib.rs`;
  only its engine tests run against the real Tari engine. The live mode
  below it is wired and ready, and activates once the template address
  is pasted into `demo/config.js`.
- **The template is not published live.** No live Esmeralda deployment
  exists yet; the live path needs a funded Sapient wallet.
- **The invisibility is the stealth property of native TARI on Ootle.**
  Amounts and owners of stealth resources are hidden from observers on
  chain. The template holds the funds in a Vault; the demo shows what
  an observer sees (nothing) against what the receiver sees after the
  reveal (the balance).
- **Epochs are the chain's clock.** An epoch is a fixed number of
  base-layer blocks. This repo does not state a duration: the docs do
  not give the block count, and the live indexer reading (epoch 13233
  on Esmeralda, 2026-11-19) is not consistent with an hour per epoch,
  so no duration is claimed here.
- **No live price or payout is promised.** The contest prizes are paid
  by the Tari Council by vote, per the contest rules.

## Sources

- Ootle docs: <https://ootle.tari.com/> (getting started, templates,
  testing, publishing, randomness, privacy in applications)
- Guessing-game walkthrough:
  <https://ootle.tari.com/guides/build-a-guessing-game/>
- Testing guide: <https://ootle.tari.com/guides/testing-templates/>
- Contest thread:
  <https://community.tari.com/t/november-build-contest-thread-mvp/520>
- Rules:
  <https://community.tari.com/t/ootle-launch-date-and-launch-contest-rules/323/1>
- Template scaffold: <https://github.com/tari-project/wasm-template>
- Precedent for the hash-commitment pattern (same contest): Ootle Bounty,
  <https://github.com/taufik52/ootle-bounty>
- dApp integration docs: <https://universe.tari.mw/integration> and its
  typed definitions at <https://universe.tari.mw/integration/tari-dapp.d.ts>
- Precedent for the on-chain demo path (same contest): Stake,
  <https://github.com/Mzee04/stake> (`web/config.js`, `web/app.js`)
