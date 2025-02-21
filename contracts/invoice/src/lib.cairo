use core::starknet::ContractAddress;

#[starknet::interface]
pub trait IInvoicePayment<TContractState> {
    /// Increase contract balance.
    fn pay_invoice(
        ref self: TContractState,
        invoice_id: u128,
        asset: ContractAddress,
        amount: u256,
        payee: ContractAddress,
    );
}


/// Simple contract for managing balance.
#[starknet::contract]
pub mod InvoicePayment {
    use starknet_types::event::EventEmitter;
    use core::starknet::{get_caller_address, ContractAddress};
    use openzeppelin_token::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};

    #[storage]
    struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        Remittance: Remittance,
    }

    #[derive(Debug, Drop, starknet::Event)]
    pub struct Remittance {
        #[key]
        pub payee: ContractAddress,
        #[key]
        pub asset: ContractAddress,
        pub invoice_id: u128,
        pub payer: ContractAddress,
        pub amount: u256,
    }

    #[abi(embed_v0)]
    impl InvoicePaymentImpl of super::IInvoicePayment<ContractState> {
        fn pay_invoice(
            ref self: ContractState,
            invoice_id: u128,
            asset: ContractAddress,
            amount: u256,
            payee: ContractAddress,
        ) {
            let payer = get_caller_address();
            let erc20_dispatcher = IERC20Dispatcher { contract_address: asset };

            assert!(erc20_dispatcher.transfer_from(payer, payee, amount));

            self.emit(Remittance { payee, asset, invoice_id, payer, amount });
        }
    }
}

