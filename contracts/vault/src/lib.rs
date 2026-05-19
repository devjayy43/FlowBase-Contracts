use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

// ─── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Balance(Address),   // per-user deposited balance
    TokenId,            // address of the accepted token
    StrategyId,         // address of the connected strategy contract
    TotalDeposits,      // aggregate deposits held by the vault
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    /// One-time setup: store the token and strategy addresses.
    pub fn initialize(env: Env, token: Address, strategy: Address) {
        if env.storage().instance().has(&DataKey::TokenId) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::TokenId, &token);
        env.storage().instance().set(&DataKey::StrategyId, &strategy);
        env.storage().instance().set(&DataKey::TotalDeposits, &0_i128);
    }

    /// Transfer `amount` tokens from `depositor` into the vault.
    pub fn deposit(env: Env, depositor: Address, amount: i128) {
        depositor.require_auth();
        assert!(amount > 0, "amount must be positive");

        let token = Self::token_client(&env);
        token.transfer(&depositor, &env.current_contract_address(), &amount);

        let balance = Self::balance_of(&env, &depositor);
        env
            .storage()
            .persistent()
            .set(&DataKey::Balance(depositor.clone()), &(balance + amount));

        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposits)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposits, &(total + amount));

        env.events().publish(
            (Symbol::new(&env, "deposit"), depositor),
            amount,
        );
    }

    /// Withdraw `amount` tokens back to `depositor`.
    pub fn withdraw(env: Env, depositor: Address, amount: i128) {
        depositor.require_auth();
        assert!(amount > 0, "amount must be positive");

        let balance = Self::balance_of(&env, &depositor);
        assert!(balance >= amount, "insufficient balance");

        env.storage()
            .persistent()
            .set(&DataKey::Balance(depositor.clone()), &(balance - amount));

        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposits)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposits, &(total - amount));

        let token = Self::token_client(&env);
        token.transfer(&env.current_contract_address(), &depositor, &amount);

        env.events().publish(
            (Symbol::new(&env, "withdraw"), depositor),
            amount,
        );
    }

    /// Push idle vault funds into the strategy to start earning yield.
    pub fn invest(env: Env, caller: Address, amount: i128) {
        caller.require_auth();
        assert!(amount > 0, "amount must be positive");

        let strategy_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::StrategyId)
            .expect("not initialized");

        let token = Self::token_client(&env);
        // Approve strategy to pull `amount` from the vault.
        token.approve(
            &env.current_contract_address(),
            &strategy_addr,
            &amount,
            &(env.ledger().sequence() + 1000),
        );

        let strategy = StrategyClient::new(&env, &strategy_addr);
        strategy.invest(&env.current_contract_address(), &amount);

        env.events().publish(
            (Symbol::new(&env, "invest"),),
            amount,
        );
    }

    /// Pull accrued yield from the strategy back into the vault.
    pub fn harvest(env: Env, caller: Address) {
        caller.require_auth();

        let strategy_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::StrategyId)
            .expect("not initialized");

        let strategy = StrategyClient::new(&env, &strategy_addr);
        let yield_amount = strategy.harvest(&env.current_contract_address());

        env.events().publish(
            (Symbol::new(&env, "harvest"),),
            yield_amount,
        );
    }

    /// Return the deposited balance for a given user.
    pub fn get_balance(env: Env, user: Address) -> i128 {
        Self::balance_of(&env, &user)
    }

    /// Return the sum of all deposits held by the vault.
    pub fn get_total_deposits(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalDeposits)
            .unwrap_or(0)
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn token_client(env: &Env) -> token::Client {
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenId)
            .expect("not initialized");
        token::Client::new(env, &token_addr)
    }

    fn balance_of(env: &Env, user: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(user.clone()))
            .unwrap_or(0)
    }
}

// ─── Strategy Interface (cross-contract calls) ────────────────────────────────

soroban_sdk::contractimport!(
    file = "../../target/wasm32-unknown-unknown/release/strategy.wasm"
);
// Exposes `StrategyClient` for cross-contract calls from the vault.
