#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
use arboard::Clipboard as SystemClipboard;

#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
use dioxus::document::eval;
#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
use serde_json::to_string;

pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    {
        let mut clipboard =
            SystemClipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| format!("Failed to copy text: {}", e))?;
        return Ok(());
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
    {
        let payload =
            to_string(text).map_err(|e| format!("Failed to encode clipboard text: {}", e))?;
        let script = format!(
            r#"
            (async function () {{
                const text = {payload};
                try {{
                    if (navigator.clipboard && window.isSecureContext) {{
                        await navigator.clipboard.writeText(text);
                        return;
                    }}
                }} catch (error) {{
                    console.warn("navigator.clipboard copy failed", error);
                }}

                try {{
                    const textarea = document.createElement("textarea");
                    textarea.value = text;
                    textarea.setAttribute("readonly", "");
                    textarea.style.position = "fixed";
                    textarea.style.opacity = "0";
                    textarea.style.pointerEvents = "none";
                    textarea.style.top = "-1000px";
                    textarea.style.left = "-1000px";
                    document.body.appendChild(textarea);
                    textarea.focus();
                    textarea.select();
                    textarea.setSelectionRange(0, textarea.value.length);
                    const copied = document.execCommand("copy");
                    document.body.removeChild(textarea);
                    if (!copied) {{
                        throw new Error("execCommand copy returned false");
                    }}
                }} catch (error) {{
                    console.error("Clipboard copy fallback failed", error);
                }}
            }})();
            "#
        );
        let _ = eval(&script);
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Clipboard copy is not supported on this platform".to_string())
}
