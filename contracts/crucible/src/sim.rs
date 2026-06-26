//! Simulated transaction dry-runs and fee estimation.

use soroban_sdk::Address;

/// An inspected transaction that allows viewing the results of a contract call
/// without the ability to commit state changes.
///
/// This type is returned by `MockEnv::simulate_inspect` and does not require
/// the closure to be `'static`, allowing it to borrow local clients and fixtures.
pub struct InspectedTx<T> {
    fee: i64,
    instructions: u64,
    required_auths: Vec<Address>,
    success: bool,
    result: Option<T>,
}

impl<T> InspectedTx<T> {
    /// Internal constructor for `MockEnv`.
    pub(crate) fn new(
        fee: i64,
        instructions: u64,
        required_auths: Vec<Address>,
        success: bool,
        result: Option<T>,
    ) -> Self {
        Self {
            fee,
            instructions,
            required_auths,
            success,
            result,
        }
    }

    /// Returns the estimated network fee in stroops.
    pub fn fee(&self) -> i64 {
        self.fee
    }

    /// Returns the total instruction count consumed by the call.
    pub fn instructions(&self) -> u64 {
        self.instructions
    }

    /// Returns the list of addresses that required authorization during the call.
    pub fn required_auths(&self) -> Vec<Address> {
        self.required_auths.clone()
    }

    /// Returns whether the transaction would succeed if committed.
    pub fn would_succeed(&self) -> bool {
        self.success
    }

    /// Returns the result of the call if it succeeded, or `None` if it failed.
    pub fn result(&self) -> Option<&T> {
        self.result.as_ref()
    }
}

/// A simulated transaction that allows inspecting the results of a contract call
/// without committing the state changes.
///
/// This type is returned by `MockEnv::simulate` and stores the closure to enable
/// the `commit()` method, which requires the closure to be `'static`.
pub struct SimulatedTx<T> {
    fee: i64,
    instructions: u64,
    required_auths: Vec<Address>,
    success: bool,
    result: Option<T>,
    re_run: Option<Box<dyn FnOnce() -> T>>,
}

impl<T> SimulatedTx<T> {
    /// Internal constructor for `MockEnv`.
    pub(crate) fn new(
        fee: i64,
        instructions: u64,
        required_auths: Vec<Address>,
        success: bool,
        result: Option<T>,
        re_run: Option<Box<dyn FnOnce() -> T>>,
    ) -> Self {
        Self {
            fee,
            instructions,
            required_auths,
            success,
            result,
            re_run,
        }
    }

    /// Returns the estimated network fee in stroops.
    pub fn fee(&self) -> i64 {
        self.fee
    }

    /// Returns the total instruction count consumed by the call.
    pub fn instructions(&self) -> u64 {
        self.instructions
    }

    /// Returns the list of addresses that required authorization during the call.
    pub fn required_auths(&self) -> Vec<Address> {
        self.required_auths.clone()
    }

    /// Returns whether the transaction would succeed if committed.
    pub fn would_succeed(&self) -> bool {
        self.success
    }

    /// Returns the result of the call if it succeeded, or `None` if it failed.
    pub fn result(&self) -> Option<&T> {
        self.result.as_ref()
    }

    /// Re-runs the call and commits the state changes.
    ///
    /// # Panics
    ///
    /// Panics if the transaction would not succeed or if `commit()` has already been called.
    pub fn commit(mut self) -> T {
        if !self.would_succeed() {
            panic!("Cannot commit a failed transaction simulation.");
        }

        let re_run = self
            .re_run
            .take()
            .expect("Transaction already committed or closure was consumed.");
        re_run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{MockEnv, MockEnvBuilder, Stroops};
    use soroban_sdk::Address;

    #[test]
    fn test_inspected_tx_borrows_local_client() {
        let env = MockEnv::builder()
            .with_account("alice", Stroops::xlm(10_000))
            .build();

        let alice = env.account("alice");
        let address = alice.address();

        // This works because simulate_inspect doesn't require 'static
        let inspected = env.simulate_inspect(|| {
            // We can borrow the address here
            address.clone()
        });

        assert!(inspected.would_succeed());
        assert_eq!(inspected.result(), Some(&address));
    }

    #[test]
    fn test_simulated_tx_requires_static() {
        let env = MockEnv::builder()
            .with_account("alice", Stroops::xlm(10_000))
            .build();

        let alice = env.account("alice");
        let address = alice.address();

        // This requires 'static, so we need to clone or use Arc
        let address_clone = address.clone();
        let sim = env.simulate(move || {
            // Must use owned data, not borrowed
            address_clone
        });

        assert!(sim.would_succeed());
        assert_eq!(sim.result(), Some(&address));
    }

    #[test]
    fn test_inspected_tx_inspection_methods() {
        let env = MockEnv::builder()
            .with_account("alice", Stroops::xlm(10_000))
            .build();

        let inspected = env.simulate_inspect(|| {
            env.account("alice").address()
        });

        assert!(inspected.would_succeed());
        assert!(inspected.fee() >= 0);
        assert!(inspected.instructions() > 0);
        assert!(inspected.result().is_some());
    }
}
