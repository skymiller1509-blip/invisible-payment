//! invisible-payment-live: drives the Invisible Payment template on Esmeralda (testnet)
//! from local test keys. No Sapient wallet, no desktop, no seed phrase from anyone.
//!
//!   live new <who>                                  make a test key (live/keys/<who>.key, mode 600)
//!   live whoami <who>                               print address, account and public key
//!   live faucet <who>                               claim free testnet TARI into <who>'s account
//!   live balance <who>                              epoch and balances
//!   live publish-dry <who> <wasm>                   dry-run the template publish, print the fee, send nothing
//!   live publish <who> <wasm>                       publish the template
//!   live hide <who> <template> <receiver> <µT> <reveal-epoch> <secret-phrase>
//!                                                     lock funds in a vault for <receiver>
//!   live reveal <who> <component> <secret-phrase>   receiver reveals the payment into their account
//!   live cancel <who> <component>                   sender cancels and reclaims before the reveal epoch
//!
//! Every transaction is dry-run first and only sent if the dry run commits.
//! Modelled on ajo-cli (github.com/G-ojies/ajo-ootle), which proved this route on Esmeralda.

use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result, anyhow, bail, ensure};
use ootle_rs::{
    Network, ToAccountAddress, TransactionRequest,
    builtin_templates::{
        UnsignedTransactionBuilder,
        component::{IComponent, TransactionBuildable},
        account::IAccount,
        faucet::IFaucet,
    },
    default_indexer_url,
    key_provider::PrivateKeyProvider,
    keys::OotleSecretKey,
    provider::{IndexerProvider, ProviderBuilder, WalletProvider},
    Address,
    template_types::{
        Amount, ComponentAddress,
        constants::TARI_TOKEN,
        crypto::RistrettoPublicKeyBytes,
    },
    wallet::OotleWallet,
};
use sha2::{Digest, Sha256};
use tari_crypto::{ristretto::RistrettoSecretKey, tari_utilities::hex::Hex};
use tari_ootle_common_types::engine_types::published_template::PublishedTemplateAddress;
use tari_ootle_transaction::{Epoch, args};

const NETWORK: Network = Network::Esmeralda;

/// Most a transaction may spend on fees: 5 TARI in µT.
const MAX_FEE: u64 = 5_000_000;

const EPOCH_WINDOW: u64 = 10;

type Provider = IndexerProvider<OotleWallet>;

/// Keys live next to this crate: live/keys/<who>.key (mode 600).
fn key_file(who: &str) -> Result<PathBuf> {
    ensure!(who.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "names are letters, digits and -");
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("keys").join(format!("{who}.key")))
}

fn new_key(who: &str) -> Result<()> {
    let path = key_file(who)?;
    fs::create_dir_all(path.parent().unwrap())?;
    let secret = OotleSecretKey::random(NETWORK);
    let mut f = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)
        .with_context(|| format!("{} already exists", path.display()))?;
    writeln!(f, "{}\n{}", secret.account_secret().to_hex(), secret.view_only_secret().to_hex())?;
    println!("{who}: {}", secret.to_address());
    Ok(())
}

fn load_key(who: &str) -> Result<OotleSecretKey> {
    let path = key_file(who)?;
    let text = fs::read_to_string(&path).with_context(|| format!("no key for {who}; run `live new {who}`"))?;
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    ensure!(lines.len() == 2, "malformed key file {}", path.display());
    let parse = |h: &str| RistrettoSecretKey::from_hex(h).map_err(|_| anyhow!("malformed key file"));
    Ok(OotleSecretKey::new(NETWORK, parse(lines[0])?, parse(lines[1])?))
}

struct Who {
    account: ComponentAddress,
    provider: Provider,
    epoch: u64,
}

async fn connect(who: &str) -> Result<Who> {
    let secret = load_key(who)?;
    let address = secret.to_address();
    let account = address.to_account_address();
    let provider = ProviderBuilder::new()
        .wallet(OotleWallet::from(PrivateKeyProvider::new(secret)))
        .connect_with_transaction_timeout(default_indexer_url(NETWORK), Duration::from_secs(240))
        .await
        .context("connect to the Esmeralda indexer")?;
    let epoch = provider.get_epoch().await?.as_u64();
    Ok(Who { account, provider, epoch })
}

fn public_key_of(who: &str) -> Result<RistrettoPublicKeyBytes> {
    Ok(*load_key(who)?.to_address().account_public_key())
}

/// Dry-runs, then (unless dry_only) sends and waits for the commit.
/// `spend` is TARI the instructions withdraw.
async fn run<F>(w: &mut Who, spend: Amount, dry_only: bool, build: F) -> Result<()>
where
    F: FnOnce(ootle_rs::builtin_templates::component::ComponentInvokeBuilder<'_, Provider>)
        -> ootle_rs::builtin_templates::component::ComponentInvokeBuilder<'_, Provider>,
{
    let tari = w.provider.get_account_balance(w.account, TARI_TOKEN).await?;
    let available = tari.checked_sub(spend).filter(Amount::is_positive)
        .ok_or_else(|| anyhow!("account holds {tari} µT, not enough for {spend} µT plus fees"))?;
    let reserve = available.to_u64_checked().map_or(MAX_FEE, |a| a.min(MAX_FEE));
    let nonce: u64 = rand::random();
    let max_epoch = Epoch(w.epoch + EPOCH_WINDOW);
    let provider = &mut w.provider;
    let tx = build(IComponent::new(provider, max_epoch).pay_fee(reserve).then(|t| t.with_nonce(nonce)))
        .prepare()
        .await?;
    let dry = provider.sign_and_send_dry_run(tx.clone()).await?;
    if let Some(reason) = dry.finalize.any_reject() {
        bail!("refused in dry run: {reason}");
    }
    let fee = dry.finalize.required_fees();
    println!("dry run fee {} µT", fee);
    ensure!(fee <= reserve, "needs {fee} µT in fees, only {reserve} µT reserved");
    if dry_only {
        println!("dry run only, nothing sent");
        return Ok(());
    }
    let signed = TransactionRequest::default().with_transaction(tx).build(provider.wallet()).await?;
    let pending = provider.send_transaction(signed).await?;
    println!("tx {}", pending.tx_id());
    let outcome = pending.watch().await?;
    ensure!(outcome.is_commit(), "transaction {} did not commit: {outcome:?}", pending.tx_id());
    let receipt = pending.get_receipt().await?;
    println!("committed in epoch {}, fee {} µT", receipt.epoch, receipt.fee_receipt.total_fees_paid());
    for up in &receipt.diff_summary.upped {
        let id = up.substate_id.to_string();
        if id.starts_with("component_") || id.starts_with("template_") {
            println!("upped {id}");
        }
    }
    for e in &receipt.events {
        println!("event {} {{{}}}", e.topic(), e.payload());
    }
    Ok(())
}

async fn faucet(w: &mut Who) -> Result<()> {
    let max_epoch = Epoch(w.epoch + EPOCH_WINDOW);
    let tx = IFaucet::new(&w.provider, max_epoch).take_faucet_funds().pay_fee(1_000u64).prepare().await?;
    let signed = TransactionRequest::default().with_transaction(tx).build(w.provider.wallet()).await?;
    let pending = w.provider.send_transaction(signed).await?;
    println!("tx {}", pending.tx_id());
    let outcome = pending.watch().await?;
    ensure!(outcome.is_commit(), "faucet claim did not commit: {outcome:?}");
    println!("balance {} µT", w.provider.get_account_balance(w.account, TARI_TOKEN).await?);
    Ok(())
}

fn template(s: &str) -> Result<ootle_rs::template_types::TemplateAddress> {
    Ok(PublishedTemplateAddress::from_str(s).map_err(|e| anyhow!("bad template address: {e:?}"))?.as_template_address())
}

fn component(s: &str) -> Result<ComponentAddress> {
    ComponentAddress::from_str(s).map_err(|e| anyhow!("bad component address: {e:?}"))
}

fn micro(s: &str) -> Result<Amount> {
    Ok(Amount::new(s.parse::<u128>().context("amount in µT")?))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["new", who] => new_key(who)?,

        ["whoami", who] => {
            let addr = load_key(who)?.to_address();
            println!("address    {addr}\naccount    {}\npublic key {}", addr.to_account_address(), addr.account_public_key());
        },

        ["faucet", who] => faucet(&mut connect(who).await?).await?,

        ["balance", who] => {
            let w = connect(who).await?;
            println!("epoch {}", w.epoch);
            for (r, b) in w.provider.get_account_balances(w.account).await? {
                println!("{r}: {b}");
            }
        },

        ["publish-dry", who, wasm] => {
            let binary = fs::read(wasm).with_context(|| format!("read {wasm}"))?;
            println!("template {} bytes", binary.len());
            let mut w = connect(who).await?;
            run(&mut w, Amount::zero(), true, |b| b.then(|t| t.publish_template(binary))).await?;
        },

        ["publish", who, wasm] => {
            let binary = fs::read(wasm).with_context(|| format!("read {wasm}"))?;
            println!("template {} bytes", binary.len());
            let mut w = connect(who).await?;
            run(&mut w, Amount::zero(), false, |b| b.then(|t| t.publish_template(binary))).await?;
        },

        ["hide", who, tpl, receiver, amount, reveal_epoch, secret] => {
            let tpl = template(tpl)?;
            let receiver_key = public_key_of(receiver)?;
            let amount = micro(amount)?;
            let reveal_epoch: u64 = reveal_epoch.parse().context("reveal epoch must be a number")?;
            let secret_hash = Sha256::digest(secret.as_bytes()).to_vec();
            let mut w = connect(who).await?;
            let account = w.account;
            run(&mut w, amount, false, |b| {
                b.call_method(account, "withdraw", args![TARI_TOKEN, amount])
                    .put_last_instruction_output_on_workspace("funds")
                    .call_function(tpl, "create", args![Workspace("funds"), receiver_key, reveal_epoch, secret_hash])
            })
            .await?;
        },

        ["reveal", who, payment, secret] => {
            let payment = component(payment)?;
            let mut w = connect(who).await?;
            let account = w.account;
            run(&mut w, Amount::zero(), false, |b| {
                b.call_method(payment, "claim", args![secret.as_bytes().to_vec(), account])
            })
            .await?;
        },

        ["cancel", who, payment] => {
            let payment = component(payment)?;
            let mut w = connect(who).await?;
            let account = w.account;
            run(&mut w, Amount::zero(), false, |b| {
                b.call_method(payment, "cancel", args![])
                    .put_last_instruction_output_on_workspace("refund")
                    .call_method(account, "deposit", args![Workspace("refund")])
            })
            .await?;
        },

        _ => bail!("see the usage at the top of live/src/main.rs"),
    }
    Ok(())
}
