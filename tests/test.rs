// Engine tests for the InvisiblePayment template.
//
// These run against the real Tari engine in-process
// (tari_template_test_tooling), no mocks, no network. The locked funds
// are XTR from the harness's built-in faucet; on the network the funds
// are TARI, a stealth resource whose amount and owner are hidden from
// observers until the receiver reveals the payment.

use sha2::{Digest, Sha256};
use tari_template_lib::types::{
    Amount,
    ComponentAddress,
    constants::TARI_TOKEN,
    crypto::RistrettoPublicKeyBytes,
};
use tari_template_test_tooling::{
    TemplateTest,
    byte_type::ToByteType,
    crypto::{RistrettoPublicKey, RistrettoSecretKey},
    engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId},
    support::assert_error::assert_reject_reason,
    transaction::args,
};

const TEMPLATE_NAME: &str = "InvisiblePayment";
/// Locked per payment, in µTari (100 TARI; funded accounts hold 1_000 TARI).
const POOL: u64 = 100_000_000;
/// Reveal epoch used by most tests (harness starts at epoch 0).
const REVEAL_EPOCH: u64 = 10;
/// The secret phrase. Only its SHA-256 ever touches the chain.
const SECRET: &[u8] = b"only-the-receiver-knows-this-phrase";
/// A wrong phrase, for the negative tests.
const WRONG_SECRET: &[u8] = b"a-wrong-phrase";

/// Amount in µTari. (`Amount::new` takes u128.)
fn amt(v: u64) -> Amount {
    Amount::new(u128::from(v))
}

/// SHA-256 of the real secret, as the sender's client computes it.
fn secret_hash() -> Vec<u8> {
    Sha256::digest(SECRET).to_vec()
}

struct Actor {
    account: ComponentAddress,
    proof: tari_template_lib::types::NonFungibleAddress,
    secret: RistrettoSecretKey,
    public_key: RistrettoPublicKey,
}

impl Actor {
    fn public_key_bytes(&self) -> RistrettoPublicKeyBytes {
        self.public_key.to_byte_type()
    }
}

struct PaymentSetup {
    payment: ComponentAddress,
    sender: Actor,
    receiver: Actor,
    other: Actor,
}

fn funded_actor(test: &mut TemplateTest) -> Actor {
    let (account, proof, secret, public_key) = test.create_funded_account_with_keypair();
    Actor { account, proof, secret, public_key }
}

/// Creates a hidden payment with POOL from the sender's faucet-funded account.
fn setup_payment(test: &mut TemplateTest, reveal_epoch: u64) -> PaymentSetup {
    let sender = funded_actor(test);
    let receiver = funded_actor(test);
    let other = funded_actor(test);
    let template = test.get_template_address(TEMPLATE_NAME);

    test.execute_expect_success(
        test.transaction()
            .call_method(sender.account, "withdraw", args![TARI_TOKEN, POOL])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![
                    Workspace("funds"),
                    receiver.public_key_bytes(),
                    reveal_epoch,
                    secret_hash(),
                ],
            )
            .build_and_seal(&sender.secret),
        vec![sender.proof.clone()],
    );
    // The constructor's return value does not decode as a plain address via
    // expect_return, so (as in the official guessing-game test) find the
    // component through the state store instead.
    let (payment, _) = test
        .read_only_state_store()
        .get_components_by_template_address(template)
        .unwrap()
        .remove(0);

    PaymentSetup { payment, sender, receiver, other }
}

fn vault_balance(test: &mut TemplateTest, payment: ComponentAddress) -> Amount {
    test.call_method(payment, "vault_balance", args![], vec![])
}

fn vault_status(test: &mut TemplateTest, payment: ComponentAddress) -> u8 {
    test.call_method(payment, "vault_status", args![], vec![])
}

fn account_balance(test: &mut TemplateTest, actor: &Actor) -> Amount {
    test.call_method(
        actor.account,
        "balance",
        args![TARI_TOKEN],
        vec![actor.proof.clone()],
    )
}

fn set_epoch(test: &mut TemplateTest, epoch: u64) {
    test.set_virtual_substate(VirtualSubstateId::CurrentEpoch, VirtualSubstate::CurrentEpoch(epoch));
}

#[test]
fn create_locks_funds_and_records_terms() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // The vault holds the full deposit.
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));

    // Status is Hidden (0).
    assert_eq!(vault_status(&mut test, setup.payment), 0);

    // The recorded terms match what the sender set.
    let reveal: u64 = test.call_method(setup.payment, "reveal_epoch", args![], vec![]);
    assert_eq!(reveal, REVEAL_EPOCH);
    let receiver_key: RistrettoPublicKeyBytes =
        test.call_method(setup.payment, "receiver_key", args![], vec![]);
    assert_eq!(receiver_key, setup.receiver.public_key_bytes());
    let sender_key: RistrettoPublicKeyBytes =
        test.call_method(setup.payment, "sender_key", args![], vec![]);
    assert_eq!(sender_key, setup.sender.public_key_bytes());
    let stored_hash: Vec<u8> =
        test.call_method(setup.payment, "secret_hash", args![], vec![]);
    assert_eq!(stored_hash, secret_hash());

    // The sender funded the payment from their own account.
    assert_eq!(
        account_balance(&mut test, &setup.sender),
        Amount::from(TemplateTest::FUNDED_ACCOUNT_INITIAL_BALANCE) - amt(POOL)
    );
}

#[test]
fn create_rejects_short_hash() {
    let mut test = TemplateTest::my_crate();
    let sender = funded_actor(&mut test);
    let receiver = funded_actor(&mut test);
    let template = test.get_template_address(TEMPLATE_NAME);

    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(sender.account, "withdraw", args![TARI_TOKEN, POOL])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![
                    Workspace("funds"),
                    receiver.public_key_bytes(),
                    REVEAL_EPOCH,
                    vec![1u8; 16],
                ],
            )
            .build_and_seal(&sender.secret),
        vec![sender.proof.clone()],
    );
    assert_reject_reason(reason, "Secret hash must be 32 bytes");
}

#[test]
fn create_rejects_past_reveal_epoch() {
    let mut test = TemplateTest::my_crate();
    let sender = funded_actor(&mut test);
    let receiver = funded_actor(&mut test);
    let template = test.get_template_address(TEMPLATE_NAME);

    // The harness starts at epoch 0, so a reveal epoch of 0 is not in
    // the future.
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(sender.account, "withdraw", args![TARI_TOKEN, POOL])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![Workspace("funds"), receiver.public_key_bytes(), 0u64, secret_hash(),],
            )
            .build_and_seal(&sender.secret),
        vec![sender.proof.clone()],
    );
    assert_reject_reason(reason, "Reveal epoch must be in the future");
}

#[test]
fn claim_blocked_before_reveal_epoch() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // Epoch is still 0, reveal is 10: even the receiver with the right
    // secret cannot reveal yet.
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![SECRET.to_vec(), setup.receiver.account],
            )
            .build_and_seal(&setup.receiver.secret),
        vec![],
    );
    assert_reject_reason(reason, "Reveal epoch has not passed yet");

    // Nothing moved: the payment is still hidden and full.
    assert_eq!(vault_status(&mut test, setup.payment), 0);
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));
}

#[test]
fn claim_blocked_with_wrong_secret() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // Move past the reveal epoch, then claim with the wrong phrase.
    set_epoch(&mut test, REVEAL_EPOCH);
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![WRONG_SECRET.to_vec(), setup.receiver.account],
            )
            .build_and_seal(&setup.receiver.secret),
        vec![],
    );
    assert_reject_reason(reason, "Secret does not match");

    // Nothing moved: the payment is still hidden and full.
    assert_eq!(vault_status(&mut test, setup.payment), 0);
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));
}

#[test]
fn claim_blocked_for_non_receiver() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // Past the reveal epoch, with the right secret — but signed by a
    // stranger, not the receiver.
    set_epoch(&mut test, REVEAL_EPOCH);
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![SECRET.to_vec(), setup.other.account],
            )
            .build_and_seal(&setup.other.secret),
        vec![],
    );
    assert_reject_reason(reason, "Only the receiver can claim this payment");

    // Nothing moved.
    assert_eq!(vault_status(&mut test, setup.payment), 0);
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));
}

#[test]
fn claim_succeeds_after_epoch_with_right_secret() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // Move past the reveal epoch, then claim with the right secret.
    set_epoch(&mut test, REVEAL_EPOCH);
    let result = test.execute_expect_success(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![SECRET.to_vec(), setup.receiver.account],
            )
            .build_and_seal(&setup.receiver.secret),
        vec![],
    );

    // The template emitted the reveal event.
    let event = result
        .finalize
        .events
        .iter()
        .find(|event| event.topic() == "InvisiblePayment.PaymentRevealed")
        .expect("PaymentRevealed event not found");
    assert_eq!(
        event.payload().get_str("amount").expect("amount in event"),
        POOL.to_string()
    );

    // The payment is revealed and empty.
    assert_eq!(vault_status(&mut test, setup.payment), 1);
    assert_eq!(vault_balance(&mut test, setup.payment), Amount::zero());

    // The receiver really received the funds: 1_000 funded + 100 revealed.
    assert_eq!(
        account_balance(&mut test, &setup.receiver),
        Amount::from(TemplateTest::FUNDED_ACCOUNT_INITIAL_BALANCE) + amt(POOL)
    );

    // A second claim is rejected: the payment is spent.
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![SECRET.to_vec(), setup.receiver.account],
            )
            .build_and_seal(&setup.receiver.secret),
        vec![],
    );
    assert_reject_reason(reason, "Payment is not active");
}

#[test]
fn cancel_returns_funds_before_epoch() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // The sender cancels at epoch 0 and takes the refund back.
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.payment, "cancel", args![])
            .put_last_instruction_output_on_workspace("refund")
            .call_method(setup.sender.account, "deposit", args![Workspace("refund")])
            .build_and_seal(&setup.sender.secret),
        vec![setup.sender.proof.clone()],
    );

    // The payment is cancelled and empty.
    assert_eq!(vault_status(&mut test, setup.payment), 2);
    assert_eq!(vault_balance(&mut test, setup.payment), Amount::zero());

    // The sender is whole again: 1_000 funded - 100 locked + 100 refunded.
    assert_eq!(
        account_balance(&mut test, &setup.sender),
        Amount::from(TemplateTest::FUNDED_ACCOUNT_INITIAL_BALANCE)
    );

    // The receiver can no longer reveal a cancelled payment.
    set_epoch(&mut test, REVEAL_EPOCH);
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(
                setup.payment,
                "claim",
                args![SECRET.to_vec(), setup.receiver.account],
            )
            .build_and_seal(&setup.receiver.secret),
        vec![],
    );
    assert_reject_reason(reason, "Payment is not active");
}

#[test]
fn cancel_blocked_after_reveal_epoch() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // Past the reveal epoch, the sender can no longer take the funds back.
    set_epoch(&mut test, REVEAL_EPOCH);
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.payment, "cancel", args![])
            .build_and_seal(&setup.sender.secret),
        vec![setup.sender.proof.clone()],
    );
    assert_reject_reason(reason, "Payment can no longer be cancelled after the reveal epoch");

    // The payment is still hidden and full, and the receiver can still reveal it.
    assert_eq!(vault_status(&mut test, setup.payment), 0);
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));
}

#[test]
fn cancel_blocked_for_non_sender() {
    let mut test = TemplateTest::my_crate();
    let setup = setup_payment(&mut test, REVEAL_EPOCH);

    // A stranger tries to cancel. The component owner rule denies
    // non-owners before the method body runs.
    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.payment, "cancel", args![])
            .build_and_seal(&setup.other.secret),
        vec![],
    );
    assert_reject_reason(reason, "AccessDenied");

    // The payment is untouched.
    assert_eq!(vault_status(&mut test, setup.payment), 0);
    assert_eq!(vault_balance(&mut test, setup.payment), amt(POOL));
}
