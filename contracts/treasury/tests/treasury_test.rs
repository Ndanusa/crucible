#[cfg(test)]
mod tests {
    use super::*;
    use crucible::prelude::*;
    use soroban_sdk::{Address, Symbol, Vec};

    fn setup_env() -> MockEnv {
        MockEnv::builder()
            .with_contract::<Treasury>()
            .with_token::<MockToken>() // ensure token availability
            .build()
    }

    #[test]
    fn test_initialize_and_admins() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admin2 = env.account("admin2");
        let admins = Vec::from_array(&env, &[admin1.address(), admin2.address()]);
        Treasury::initialize(env.inner(), admins.clone(), 2);
        // prepare a token and fund depositor
        let token = MockToken::xlm(&env);
        token.mint(&admin1.address(), 1_000);
        // deposit by admin1
        Treasury::deposit(env.inner(), admin1.address(), token.address(), 1_000);
        let bal = Treasury::balance_of(env.inner(), env.current_contract_address(), token.address());
        assert_eq!(bal, 1_000);
    }

    #[test]
    #[should_panic]
    fn test_withdraw_insufficient_quorum() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admin2 = env.account("admin2");
        let admins = Vec::from_array(&env, &[admin1.address(), admin2.address()]);
        Treasury::initialize(env.inner(), admins.clone(), 2);
        let token = MockToken::xlm(&env);
        token.mint(&admin1.address(), 1_000);
        Treasury::deposit(env.inner(), admin1.address(), token.address(), 1_000);
        // attempt withdraw with only one signer
        Treasury::withdraw(
            env.inner(),
            admin1.address(),
            token.address(),
            500,
            Vec::from_array(&env, &[admin1.address()]),
        );
    }

    #[test]
    fn test_successful_withdraw() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admin2 = env.account("admin2");
        let admins = Vec::from_array(&env, &[admin1.address(), admin2.address()]);
        Treasury::initialize(env.inner(), admins.clone(), 2);
        let token = MockToken::xlm(&env);
        token.mint(&admin1.address(), 1_000);
        Treasury::deposit(env.inner(), admin1.address(), token.address(), 1_000);
        // withdraw with both admins
        Treasury::withdraw(
            env.inner(),
            admin1.address(),
            token.address(),
            400,
            Vec::from_array(&env, &[admin1.address(), admin2.address()]),
        );
        let bal = Treasury::balance_of(env.inner(), env.current_contract_address(), token.address());
        assert_eq!(bal, 600);
    }

    // --- invalid initialization tests ---

    #[test]
    #[should_panic]
    fn test_initialize_empty_admins_rejected() {
        let env = setup_env();
        Treasury::initialize(env.inner(), Vec::new(&env.inner()), 1);
    }

    #[test]
    #[should_panic]
    fn test_initialize_zero_quorum_rejected() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admins = Vec::from_array(&env, &[admin1.address()]);
        Treasury::initialize(env.inner(), admins, 0);
    }

    #[test]
    #[should_panic]
    fn test_initialize_quorum_exceeds_admin_count_rejected() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admins = Vec::from_array(&env, &[admin1.address()]);
        Treasury::initialize(env.inner(), admins, 2);
    }

    #[test]
    #[should_panic]
    fn test_initialize_duplicate_admins_rejected() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let addr = admin1.address();
        let admins = Vec::from_array(&env, &[addr.clone(), addr]);
        Treasury::initialize(env.inner(), admins, 1);
    }

    #[test]
    #[should_panic]
    fn test_initialize_twice_rejected() {
        let env = setup_env();
        let admin1 = env.account("admin1");
        let admins = Vec::from_array(&env, &[admin1.address()]);
        Treasury::initialize(env.inner(), admins.clone(), 1);
        Treasury::initialize(env.inner(), admins, 1);
    }
}
