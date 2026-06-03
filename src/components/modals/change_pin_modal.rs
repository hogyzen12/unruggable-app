use crate::components::pin_input::PinInput;
use crate::storage;
use crate::AppSecurityContext;
use dioxus::prelude::*;

const CHANGE_PIN_LOCK_ART: Asset = asset!("/assets/lock.png");

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChangePinStage {
    Current,
    New,
    Confirm,
}

#[component]
pub fn ChangePinModal(onclose: EventHandler<()>) -> Element {
    let app_security = use_context::<AppSecurityContext>();

    let mut stage = use_signal(|| ChangePinStage::Current);
    let mut stage_revision = use_signal(|| 0u64);
    let mut current_pin = use_signal(String::new);
    let mut next_pin = use_signal(String::new);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut saving = use_signal(|| false);

    let reset_key = {
        let stage_key = match stage() {
            ChangePinStage::Current => "current",
            ChangePinStage::New => "new",
            ChangePinStage::Confirm => "confirm",
        };
        format!("change-pin-{stage_key}-{}", stage_revision())
    };

    let pin_title = match stage() {
        ChangePinStage::Current => "Current PIN",
        ChangePinStage::New => "Create New PIN",
        ChangePinStage::Confirm => "Confirm New PIN",
    }
    .to_string();

    let pin_subtitle = match stage() {
        ChangePinStage::Current => {
            "Enter your current 6-digit PIN before changing the wallet encryption key."
        }
        ChangePinStage::New => "Choose a new 6-digit PIN for this device.",
        ChangePinStage::Confirm => "Enter the same new PIN again to finish the change.",
    }
    .to_string();

    let step_indicator = match stage() {
        ChangePinStage::Current => "Step 1 of 3",
        ChangePinStage::New => "Step 2 of 3",
        ChangePinStage::Confirm => "Step 3 of 3",
    }
    .to_string();

    let note_label = match stage() {
        ChangePinStage::Current => "Security check",
        ChangePinStage::New => "New device PIN",
        ChangePinStage::Confirm => "One more check",
    }
    .to_string();

    let note_copy = match stage() {
        ChangePinStage::Current => {
            "Your current PIN is required before the wallet secrets can be re-encrypted."
        }
        ChangePinStage::New => {
            "This new PIN will protect the encrypted wallet data already stored on this device."
        }
        ChangePinStage::Confirm => {
            "Confirm the same PIN. The wallet will be rewrapped in place without deleting anything."
        }
    }
    .to_string();

    let show_strength = matches!(stage(), ChangePinStage::New);
    let processing_label = "Changing PIN securely...".to_string();

    let handle_pin_input = EventHandler::new(move |_| {
        app_security.record_activity();
        if error_message().is_some() {
            error_message.set(None);
        }
    });

    let handle_pin_complete = move |pin: String| {
        if saving() {
            return;
        }

        app_security.record_activity();
        error_message.set(None);

        match stage() {
            ChangePinStage::Current => {
                current_pin.set(pin);
                stage.set(ChangePinStage::New);
                stage_revision += 1;
            }
            ChangePinStage::New => {
                if pin == current_pin() {
                    error_message.set(Some(
                        "New PIN must be different from your current PIN.".to_string(),
                    ));
                    return;
                }

                next_pin.set(pin);
                stage.set(ChangePinStage::Confirm);
                stage_revision += 1;
            }
            ChangePinStage::Confirm => {
                if pin != next_pin() {
                    error_message.set(Some("New PINs did not match. Try again.".to_string()));
                    next_pin.set(String::new());
                    stage.set(ChangePinStage::New);
                    stage_revision += 1;
                    return;
                }

                let current_pin_value = current_pin();
                let next_pin_value = next_pin();
                saving.set(true);

                spawn(async move {
                    match storage::change_pin_async(current_pin_value, next_pin_value).await {
                        Ok(()) => {
                            saving.set(false);
                            error_message.set(None);
                            app_security.record_activity();
                            onclose.call(());
                        }
                        Err(error) => {
                            saving.set(false);
                            error_message.set(Some(error));

                            if storage::is_pin_locked() {
                                app_security.lock_now("change pin locked");
                                onclose.call(());
                                return;
                            }

                            stage.set(ChangePinStage::Current);
                            current_pin.set(String::new());
                            next_pin.set(String::new());
                            stage_revision += 1;
                        }
                    }
                });
            }
        }
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| {
                if !saving() {
                    onclose.call(());
                }
            },

            div {
                class: "modal-content app-modal-shell change-pin-modal-shell",
                onclick: move |e| e.stop_propagation(),

                div { class: "app-modal-header",
                    h2 { class: "app-modal-title", "Change PIN" }
                    button {
                        class: "app-modal-close-button",
                        disabled: saving(),
                        onclick: move |_| {
                            if !saving() {
                                onclose.call(());
                            }
                        },
                        "×"
                    }
                }

                div { class: "modal-body change-pin-modal-body",
                    div { class: "change-pin-modal-stage onboarding-pin-shell",
                        div {
                            class: "onboarding-mini-art-shell onboarding-mini-art-shell-lock",
                            img {
                                class: "onboarding-mini-art",
                                src: CHANGE_PIN_LOCK_ART,
                                alt: "Unruggable lock artwork"
                            }
                        }

                        p {
                            class: "onboarding-description onboarding-description-tight change-pin-description",
                            "Update your wallet PIN without recreating or deleting the encrypted wallet data on this device."
                        }

                        div {
                            class: "flow-note-card flow-note-card-accent change-pin-note-card",
                            div { class: "flow-note-label", "{note_label}" }
                            p { class: "flow-note-copy", "{note_copy}" }
                        }

                        PinInput {
                            title: pin_title,
                            subtitle: Some(pin_subtitle),
                            error_message: error_message().clone(),
                            on_complete: handle_pin_complete,
                            on_cancel: None,
                            on_input: Some(handle_pin_input),
                            show_strength: Some(show_strength),
                            step_indicator: Some(step_indicator),
                            clear_on_complete: Some(true),
                            is_processing: Some(saving()),
                            processing_label: Some(processing_label),
                            reset_key: Some(reset_key),
                        }
                    }
                }
            }
        }
    }
}
