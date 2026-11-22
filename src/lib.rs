// The Invisible Payment — a Tari Ootle template.
//
// A sender locks TARI in a vault nobody can see. On Ootle, native TARI
// is a stealth resource: the amount and the owner are hidden from
// observers. The payment opens only after a reveal epoch, only to the
// named receiver, and only with the secret phrase. Until the receiver
// reveals it with the secret, there is nothing to see — no amount, no
// owner, no receiver. Then the balance appears.
//
// The lock is three real conditions — time (AfterEpoch, read as the
// current epoch), secret (SHA-256 of the phrase), receiver's key — and
// the reveal is the moment the money becomes visible. The secret itself
// never touches the chain; only its hash does.
//
// Flow:
//   1. Sender hides a payment: deposits TARI, names the receiver's
//      public key, sets a reveal epoch, stores SHA-256(secret).
//   2. Sender can cancel any time before the reveal epoch and reclaim.
//   3. After the reveal epoch, the receiver claims with the secret. The
//      template checks the epoch, checks the caller's key is the
//      receiver's, checks SHA-256(secret) against the stored hash, then
//      pays the receiver's account in full — and the payment becomes
//      visible as a balance.
//   4. The secret is hashed inside the template with the `sha2` crate
//      (pure Rust, compiles to WASM); the caller passes the phrase.
//
// Patterns follow the official guessing-game walkthrough
// (ootle.tari.com/guides/build-a-guessing-game) and the Ootle Bounty
// November contest entry: identities are the transaction signer's public
// key, `claim` is allow_all with in-body checks, sender-only methods
// rely on the default owner rule plus an in-body check.

use sha2::{Digest, Sha256};
use tari_template_lib::prelude::*;

#[template]
pub mod invisible_payment {
    use super::*;

    /// Payment has not been revealed or cancelled yet.
    const PAYMENT_HIDDEN: u8 = 0;
    /// The receiver revealed the payment.
    const PAYMENT_REVEALED: u8 = 1;
    /// The sender cancelled and reclaimed the funds.
    const PAYMENT_CANCELLED: u8 = 2;

    /// The invisible payment component.
    pub struct InvisiblePayment {
        /// Public key of the transaction signer that hid the payment.
        pub sender: RistrettoPublicKeyBytes,
        /// Public key of the receiver. Only this key can reveal it.
        pub receiver: RistrettoPublicKeyBytes,
        /// Locked TARI. On the network this is a stealth resource:
        /// the amount is hidden from observers.
        pub funds: Vault,
        /// First epoch at which the receiver may reveal the payment.
        pub reveal_epoch: u64,
        /// SHA-256 of the secret phrase. The phrase itself is never stored.
        pub secret_hash: Vec<u8>,
        /// 0 = Hidden, 1 = Revealed, 2 = Cancelled.
        pub status: u8,
    }

    impl InvisiblePayment {
        /// Hides a payment, locking `funds` for `receiver` until
        /// `reveal_epoch`. The caller is the sender.
        ///
        /// # Panics
        ///
        /// Panics if `secret_hash` is not 32 bytes or if `reveal_epoch`
        /// is not in the future.
        pub fn create(
            funds: Bucket,
            receiver: RistrettoPublicKeyBytes,
            reveal_epoch: u64,
            secret_hash: Vec<u8>,
        ) -> Component<Self> {
            assert_eq!(secret_hash.len(), 32, "Secret hash must be 32 bytes");
            assert!(
                reveal_epoch > Consensus::current_epoch(),
                "Reveal epoch must be in the future"
            );

            let sender = CallerContext::transaction_signer_public_key();

            // Public entry points: anyone may attempt a reveal (the
            // in-body checks decide), and anyone may read state. Cancel
            // is left under the default rule (deny_all), so only the
            // component owner — the sender that created it — may call it.
            let access_rules = ComponentAccessRules::new()
                .method("claim", rule![allow_all])
                .method("vault_balance", rule![allow_all])
                .method("vault_status", rule![allow_all])
                .method("reveal_epoch", rule![allow_all])
                .method("receiver_key", rule![allow_all])
                .method("sender_key", rule![allow_all])
                .method("secret_hash", rule![allow_all]);

            let component = Component::new(Self {
                sender,
                receiver,
                funds: Vault::from_bucket(funds),
                reveal_epoch,
                secret_hash,
                status: PAYMENT_HIDDEN,
            })
            .with_access_rules(access_rules)
            .create();

            emit_event(
                "PaymentHidden",
                metadata![
                    "sender" => sender.to_string(),
                    "receiver" => receiver.to_string(),
                    "reveal_epoch" => reveal_epoch.to_string(),
                ],
            );

            component
        }

        /// Reveals the payment for the receiver.
        ///
        /// Callable by: anyone, but only the receiver's signature passes.
        ///
        /// # Panics
        ///
        /// Panics if the payment is not hidden, the reveal epoch has not
        /// passed yet, the caller is not the receiver, or the secret
        /// does not hash to the stored hash.
        pub fn claim(&mut self, secret: Vec<u8>, payout_to: ComponentAddress) {
            assert_eq!(self.status, PAYMENT_HIDDEN, "Payment is not active");
            assert!(
                Consensus::current_epoch() >= self.reveal_epoch,
                "Reveal epoch has not passed yet"
            );

            let caller = CallerContext::transaction_signer_public_key();
            assert_eq!(
                caller, self.receiver,
                "Only the receiver can claim this payment"
            );

            let computed = Sha256::digest(&secret);
            assert_eq!(
                computed.as_slice(),
                self.secret_hash.as_slice(),
                "Secret does not match"
            );

            self.status = PAYMENT_REVEALED;
            let amount = self.funds.balance();
            let payout = self.funds.withdraw(amount);

            // Pay the receiver's account in full. This is a
            // cross-component call that invokes `deposit` on the
            // receiver's account, the same pattern the official
            // guessing game uses to pay a winner.
            ComponentManager::get(payout_to).invoke("deposit", args![payout]);

            emit_event(
                "PaymentRevealed",
                metadata![
                    "receiver" => caller.to_string(),
                    "amount" => amount.to_string(),
                ],
            );
        }

        /// Cancels the payment and returns the funds to the sender.
        ///
        /// Callable by: the sender only (owner rule + in-body check),
        /// and only before the reveal epoch, so the lock means something.
        ///
        /// # Panics
        ///
        /// Panics if the caller is not the sender, the payment is not
        /// hidden, or the reveal epoch has already passed.
        pub fn cancel(&mut self) -> Bucket {
            assert_eq!(
                CallerContext::transaction_signer_public_key(),
                self.sender,
                "Only the sender can cancel this payment"
            );
            assert_eq!(self.status, PAYMENT_HIDDEN, "Payment is not active");
            assert!(
                Consensus::current_epoch() < self.reveal_epoch,
                "Payment can no longer be cancelled after the reveal epoch"
            );

            self.status = PAYMENT_CANCELLED;
            let amount = self.funds.balance();

            emit_event("PaymentCancelled", metadata!["amount" => amount.to_string()]);

            self.funds.withdraw(amount)
        }

        // ---- Read-only getters (allow_all) ----

        /// Current locked balance.
        pub fn vault_balance(&self) -> Amount {
            self.funds.balance()
        }

        /// Payment status: 0 = Hidden, 1 = Revealed, 2 = Cancelled.
        pub fn vault_status(&self) -> u8 {
            self.status
        }

        /// First epoch at which the receiver may reveal the payment.
        pub fn reveal_epoch(&self) -> u64 {
            self.reveal_epoch
        }

        /// Public key of the receiver.
        pub fn receiver_key(&self) -> RistrettoPublicKeyBytes {
            self.receiver
        }

        /// Public key of the sender.
        pub fn sender_key(&self) -> RistrettoPublicKeyBytes {
            self.sender
        }

        /// Stored SHA-256 of the secret phrase. The hash is public; the
        /// phrase is not.
        pub fn secret_hash(&self) -> Vec<u8> {
            self.secret_hash.clone()
        }
    }
}
