use near_runtime_fees::{
    AccessKeyCreationConfig, ActionCreationConfig, DataReceiptCreationConfig, Fee, Fraction,
    RuntimeFeesConfig, StorageUsageConfig,
};
use node_runtime::config::RuntimeConfig;
use rand::{thread_rng, RngCore};

pub fn random_config() -> RuntimeConfig {
    let mut rng = thread_rng();
    let mut random_fee = || Fee {
        send_sir: rng.next_u64() % 1000,
        send_not_sir: rng.next_u64() % 1000,
        execution: rng.next_u64() % 1000,
    };
    RuntimeConfig {
        transaction_costs: RuntimeFeesConfig {
            action_receipt_creation_config: random_fee(),
            data_receipt_creation_config: DataReceiptCreationConfig {
                base_cost: random_fee(),
                cost_per_byte: random_fee(),
            },
            action_creation_config: ActionCreationConfig {
                create_account_cost: random_fee(),
                deploy_contract_cost: random_fee(),
                deploy_contract_cost_per_byte: random_fee(),
                function_call_cost: random_fee(),
                function_call_cost_per_byte: random_fee(),
                transfer_cost: random_fee(),
                stake_cost: random_fee(),
                add_key_cost: AccessKeyCreationConfig {
                    full_access_cost: random_fee(),
                    function_call_cost: random_fee(),
                    function_call_cost_per_byte: random_fee(),
                },
                delete_key_cost: random_fee(),
                delete_account_cost: random_fee(),
            },
            storage_usage_config: StorageUsageConfig {
                account_cost: rng.next_u64() % 10000,
                data_record_cost: rng.next_u64() % 10000,
                key_cost_per_byte: rng.next_u64() % 100,
                value_cost_per_byte: rng.next_u64() % 100,
                code_cost_per_byte: rng.next_u64() % 100,
            },
            burnt_gas_reward: Fraction { numerator: rng.next_u64() % 100, denominator: 100 },
        },
        ..Default::default()
    }
}

#[test]
fn test_random_fees() {
    assert_ne!(random_config().transaction_costs, random_config().transaction_costs);
}
