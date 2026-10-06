pub mod currency_modal;
pub mod delete_wallet_modal;
pub mod export_wallet_modal;
pub mod hardware_modal;
pub mod receive_modal;
pub mod rpc_modal;
pub mod send_modal;
pub mod send_token_modal;
pub mod stake_modal;
pub mod swap_modal;
pub mod wallet_modal;

use crate::hardware::HardwareWallet;
use dioxus::prelude::spawn;
use std::sync::Arc;

/// Cancel an unapproved hardware request and close the connection. Requiring a
/// reconnect prevents a late serial response from leaking into the next action.
pub(crate) fn cancel_hardware_operation(hardware_wallet: Option<Arc<HardwareWallet>>) -> bool {
    let Some(wallet) = hardware_wallet else {
        return true;
    };
    if !wallet.cancel_current_operation() {
        return false;
    }
    spawn(async move {
        let _ = wallet.disconnect().await;
    });
    true
}

pub use delete_wallet_modal::DeleteWalletModal;
pub use export_wallet_modal::ExportWalletModal;
pub use hardware_modal::HardwareWalletModal;
pub use receive_modal::ReceiveModal;
pub use send_modal::SendModalWithHardware;
pub use send_token_modal::SendTokenModal;
pub use stake_modal::StakeModal;
pub use swap_modal::SwapModal;
pub use wallet_modal::WalletModal;
