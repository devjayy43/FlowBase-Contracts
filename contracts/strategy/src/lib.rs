use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol};

// ─── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    TokenId,        // accepted token address
    VaultId,        // the vault that is authorised to call this contract
    TotalInvested,  // principal currently held by the strategy
    AccruedYield,   // simulated yield accumulated since last harvest
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct StrategyContract;

#[contractimpl]
impl StrategyContract {
    /// One-time setup: store the token and the authorised vault address.
    pub fn initialize(env: Env, token: Address, vault: Address) {
        if env.storage().instance().has(&DataKey::TokenId) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::TokenId, &token);
        env.storage().instance().set(&DataKey::VaultId, &vault);
        env.storage().instance().set(&DataKey::TotalInvested, &0_i128);
        env.storage().instance().set(&DataKey::AccruedYield, &0_i128);
    }

    /// Accept `amount` tokens from the vault and record the principal.
    ///
    /// Only the registered vault may call this.
    pub fn invest(env: Env, from: Address, amount: i128) {
        from.require_auth();
        Self::only_vault(&env, &from);
        assert!(amount > 0, "amount must be positive");

        let token = Self::token_client(&env);
        token.transfer_from(
            &env.current_contract_address(),
            &from,
            &env.current_contract_address(),
            &amount,
        );

        let invested: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalInvested)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalInvested, &(invested + amount));

        env.events().publish(
            (Symbol::new(&env, "invest"),),
            amount,
        );
    }

    /// Return `amount` of principal to the vault.
    pub fn withdraw(env: Env, to: Address, amount: i128) -> i128 {
        to.require_auth();
        Self::only_vault(&env, &to);
        assert!(amount > 0, "amount must be positive");

        let invested: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalInvested)
            .unwrap_or(0);
        assert!(invested >= amount, "insufficient invested balance");

        env.storage()
            .instance()
            .set(&DataKey::TotalInvested, &(invested - amount));

        let token = Self::token_client(&env);
        token.transfer(&env.current_contract_address(), &to, &amount);

        env.events().publish(
            (Symbol::new(&env, "withdraw"), to),
            amount,
        );

        amount
    }

    /// Accrue simulated yield: 1 % of the current principal per call.
    ///
    /// In production this would be replaced with real DeFi protocol logic.
    pub fn accrue_yield(env: Env) {
        let invested: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalInvested)
            .unwrap_or(0);

        let new_yield = invested / 100; // 1 % per call (placeholder)

        let accrued: i128 = env
            .storage()
            .instance()
            .get(&DataKey::AccruedYield)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::AccruedYield, &(accrued + new_yield));

        env.events().publish(
            (Symbol::new(&env, "accrue"),),
            new_yield,
        );
    }

    /// Transfer all accrued yield to the vault and reset the counter.
    ///
    /// Returns the amount transferred.
    pub fn harvest(env: Env, to: Address) -> i128 {
        to.require_auth();
        Self::only_vault(&env, &to);

        let accrued: i128 = env
            .storage()
            .instance()
            .get(&DataKey::AccruedYield)
            .unwrap_or(0);

        if accrued == 0 {
            return 0;
        }

        env.storage()
            .instance()
            .set(&DataKey::AccruedYield, &0_i128);

        let token = Self::token_client(&env);
        token.transfer(&env.current_contract_address(), &to, &accrued);

        env.events().publish(
            (Symbol::new(&env, "harvest"), to),
            accrued,
        );

        accrued
    }

    /// Read-only: total principal under management.
    pub fn get_total_invested(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalInvested)
            .unwrap_or(0)
    }

    /// Read-only: yield available to harvest.
    pub fn get_accrued_yield(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::AccruedYield)
            .unwrap_or(0)
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn token_client(env: &Env) -> token::Client {
        let addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenId)
            .expect("not initialized");
        token::Client::new(env, &addr)
    }

    fn only_vault(env: &Env, caller: &Address) {
        let vault: Address = env
            .storage()
            .instance()
            .get(&DataKey::VaultId)
            .expect("not initialized");
        assert!(caller == &vault, "caller is not the vault");
    }
}
